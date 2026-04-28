// Implementation of DataSink for the rdkafka (https://crates.io/crates/rdkafka) library as well as
// for an HTTP REST API that can be used when we can't communicate directly with the broker (it's
// here because most of the logic is the same, only the low-level transport differs).
//
// Kafka is overkill for Sonar: Sonar has a low message volume per node, even if the per-cluster
// volume can be somewhat high.  Anything that could deliver a message synchronously to a broker
// with not too much overhead and reliably store-and-forward the messages from the broker to the
// eventual endpoint in an efficient manner would have been fine, especially if it had the option of
// an efficient on-cluster intermediary.  But Kafka is standard, reliable, and will do the job.
//
// Here we use the Rust rdkafka library, which sits on top of the industrial-strength C librdkafka
// library.  Both are backed by Confluent, a big player in the Kafka space.  This is far from a
// "pure Rust" solution but the current pure Rust Kafka libraries leave a lot to be desired.
//
// For the REST API:
//
// For the time being, we farm the actual sending work out to curl.
//
// Each upload posts everything that is queued within the sending window to a single address on the
// API and lets the API sort it out.  That address is literally the rest-endpoint in the config file.
//
// The upload protocol and message format are defined in ../../util/kafka-proxy/kprox.go.

#![allow(clippy::comparison_to_empty)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use crate::daemon::{Dur, Ini, Operation};
use crate::datasink::background::*;
use crate::datasink::http_upload;
use crate::datasink::DataSink;
#[cfg(debug_assertions)]
use crate::posix::time::unix_now;
use crate::systemapi::SystemAPI;
use crate::util;
use crate::util::rng::Rng;

use std::cmp::min;
use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel;

use rdkafka::client::ClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::message::DeliveryResult;
use rdkafka::producer::base_producer::ThreadedProducer;
use rdkafka::producer::{BaseProducer, BaseRecord, NoCustomPartitioner, ProducerContext};

pub struct KafkaMsg {
    pub topic: String,
    pub key: String,
    pub value: String,
}

impl Size for KafkaMsg {
    // This can return an estimate, but it's better to overestimate than underestimate.
    fn size(&self) -> usize {
        self.topic.len() + self.key.len() + self.value.len() + 20
    }
}

pub struct KafkaSink {
    outgoing_message_queue: channel::Sender<Message<KafkaMsg>>,
    producer: Option<thread::JoinHandle<()>>,
}

// `control_and_errors` is the channel on which incoming control messages from the broker and any
// errors will be posted (as Operation::Incoming(key,value), Operation::MessageDeliveryError(msg),
// or Operation::Fatal(msg), respectively).

impl KafkaSink {
    pub fn new(
        ini: &Ini,
        client_id: String,
        control_topic: String,
        control_and_errors: channel::Sender<Operation>,
    ) -> KafkaSink {
        let global = &ini.global;
        let kafka = &ini.kafka;
        let debug = &ini.debug;
        let (outgoing_message_queue, incoming_message_queue) = channel::unbounded();
        let producer = {
            let sasl_identity = kafka
                .sasl_password
                .as_ref()
                .map(|password| (global.cluster.clone(), password.clone()));
            let sending_window = kafka.sending_window.to_seconds();
            let timeout = kafka.timeout.to_seconds();
            if kafka.broker_address != "" {
                let broker = kafka.broker_address.clone();
                let ca_file = kafka.ca_file.clone();
                thread::spawn(move || {
                    kafka_producer(
                        broker,
                        client_id,
                        ca_file,
                        sasl_identity,
                        sending_window,
                        timeout,
                        incoming_message_queue,
                        control_and_errors,
                    );
                })
            } else {
                let cutoff = kafka.http_payload_limit;
                let rest_endpoint = kafka.rest_endpoint.clone();
                let http_proxy = kafka.http_proxy.clone();
                let curl_cmd = if let Some(ref curl) = ini.programs.curl_cmd {
                    curl.clone()
                } else {
                    "curl".to_string()
                };
                thread::spawn(move || {
                    kafka_http_producer(
                        cutoff,
                        &curl_cmd,
                        &rest_endpoint,
                        &http_proxy,
                        &client_id,
                        sasl_identity,
                        sending_window,
                        timeout,
                        incoming_message_queue,
                        control_and_errors,
                    );
                })
            }
        };
        KafkaSink {
            outgoing_message_queue,
            producer: Some(producer),
        }
    }
}

impl DataSink for KafkaSink {
    fn post(
        &mut self,
        _system: &dyn SystemAPI,
        topic_prefix: &Option<String>,
        cluster: &str,
        data_tag: &str,
        hostname: &str,
        value: String,
    ) {
        // The send can really only fail if the Kafka producer thread has closed the channel, and
        // that will only happen if stop() has been called, and post() should never be called after
        // stop(), so in that case ignore the error here.
        let mut topic = cluster.to_string() + "." + data_tag;
        if let Some(ref prefix) = topic_prefix {
            topic = prefix.clone() + "." + &topic;
        }
        let key = hostname.to_string();
        let _ignored = self
            .outgoing_message_queue
            .send(Message::M(KafkaMsg { topic, key, value }));
    }

    fn stop(&mut self, _system: &dyn SystemAPI) {
        let _ = self.outgoing_message_queue.send(Message::Stop);
        let mut producer = None;
        std::mem::swap(&mut producer, &mut self.producer);
        let _ = producer.unwrap().join();
    }
}

// Kafka sending logic works like this:
//
// We use a ThreadedProducer to send messages.  In this scheme, we enqueue messages and a background
// thread will poll the Kafka subsystem as necessary to make sure they are sent.  We set a message
// timeout and that is our main means of controlling the backlog.  When a message fails to be sent
// for reasons of timeout, we simply drop it.  There may be other reasons for not sending it, but
// we're not exploring that yet.  The default is 30m, long enough for interesting things to happen
// on the broker side.
//
// Compression is set to "snappy" as it's believed it's an OK choice for JSON data.
//
// Buffering time is set to 1000ms as that will give us enough time to batch everything that's
// queued up, but in normal operation the sending window will be shorter than the cadence and there
// will not be a queue.

struct SonarProducerContext {
    control_and_errors: channel::Sender<Operation>,
}

impl ClientContext for SonarProducerContext {}

const KAFKA_BUFFER_MS: u64 = 1000;

fn kafka_producer(
    broker: String,
    client_id: String,
    ca_file: Option<String>,
    sasl_identity: Option<(String, String)>,
    sending_window: u64,
    timeout: u64,
    incoming_message_queue: channel::Receiver<Message<KafkaMsg>>,
    control_and_errors: channel::Sender<Operation>,
) {
    let mut cfg = ClientConfig::new();
    cfg.set("bootstrap.servers", &broker)
        .set("client.id", &client_id)
        .set("queue.buffering.max.ms", format!("{}", KAFKA_BUFFER_MS))
        .set("message.timeout.ms", format!("{}", timeout * 1000))
        .set("compression.codec", "snappy");
    if let Some(ref filename) = ca_file {
        cfg.set("ssl.ca.location", filename)
            .set("ssl.endpoint.identification.algorithm", "none");
        if let Some((ref username, ref password)) = sasl_identity {
            cfg.set("security.protocol", "sasl_ssl")
                .set("sasl.mechanism", "PLAIN") // yeah, must be upper case...
                .set("sasl.username", username)
                .set("sasl.password", password);
        } else {
            cfg.set("security.protocol", "ssl");
        }
    }
    let producer =
        make_sender_adapter(cfg, control_and_errors.clone()).expect("Producer creation error");
    let op = KafkaBackgroundProducer {
        producer,
        sending_window,
        control_and_errors,
    };
    background_producer(incoming_message_queue, &op);
}

struct KafkaBackgroundProducer {
    producer: Box<dyn SenderAdapter>,
    sending_window: u64,
    control_and_errors: channel::Sender<Operation>,
}

impl BackgroundSender<KafkaMsg> for KafkaBackgroundProducer {
    fn send_all(&self, mut id: usize, backlog: Vec<KafkaMsg>) {
        // We always try to send everything.  Messages that fail are dropped, because the only failure
        // is failure to be enqueued - in that case, the message is probably fatally flawed.
        for msg in backlog.iter() {
            id += 1; // Always give it a new ID, even if it is later dropped.
            log::debug!("Sending to topic: {} with id {id}", msg.topic);
            match self.producer.send(
                BaseRecord::with_opaque_to(&msg.topic, id)
                    .payload(&msg.value)
                    .key(&msg.key),
            ) {
                Ok(()) => {}
                Err(m) => {
                    // An error here only means that the message could not be enqueued; sending errors
                    // are discovered in the ProducerContext.  So an error here is pretty much fatal for
                    // the message, hence we drop it.
                    let msg = format!("Message #{id}: {m}");
                    let _ = self
                        .control_and_errors
                        .send(Operation::MessageDeliveryError(msg));
                }
            }
        }
    }

    fn sending_window_s(&self) -> u64 {
        self.sending_window
    }

    fn shutdown_delay_ms(&self) -> u64 {
        KAFKA_BUFFER_MS * 2
    }

    fn batch_size(&self) -> Option<usize> {
        None
    }

    fn metadata_size(&self) -> (usize, usize) {
        (0, 0)
    }
}

impl ProducerContext for SonarProducerContext {
    type DeliveryOpaque = usize;
    fn delivery(
        &self,
        delivery_result: &DeliveryResult<'_>,
        delivery_opaque: Self::DeliveryOpaque,
    ) {
        match delivery_result {
            Ok(_) => {
                log::debug!("Sent #{delivery_opaque} successfully");
            }
            Err((e, m)) => {
                // TODO: The message could not be sent.  We could try to disambiguate here and try
                // different actions, but for now, just drop it on the floor.
                let irritant = format!("{:?}", m);
                let msg = format!(
                    "Message #{delivery_opaque} delivery error={e:.200} irritant={irritant:.200}"
                );
                let _ = self
                    .control_and_errors
                    .send(Operation::MessageDeliveryError(msg));
            }
        }
    }
}

// The SenderAdapter hides the low level output path from higher-level logic, for the purposes of
// testing: during testing, output can go to stdout and be inspected.  There's a small cost of
// indirection here during production, but we won't really notice.

trait SenderAdapter {
    fn send(&self, r: BaseRecord<String, String, usize>) -> Result<(), String>;
}

struct KafkaSender {
    producer: ThreadedProducer<SonarProducerContext, NoCustomPartitioner>,
}

impl KafkaSender {
    fn new(
        cfg: ClientConfig,
        control_and_errors: channel::Sender<Operation>,
    ) -> Result<KafkaSender, String> {
        let producer: ThreadedProducer<SonarProducerContext, NoCustomPartitioner> =
            cfg
                .create_with_context::<SonarProducerContext,
                                       ThreadedProducer<SonarProducerContext, NoCustomPartitioner>>(
                    SonarProducerContext { control_and_errors },
                )
                .map_err(|e| format!("Could not create Kafka sender, error={e}"))?;
        Ok(KafkaSender { producer })
    }
}

impl SenderAdapter for KafkaSender {
    fn send(&self, r: BaseRecord<String, String, usize>) -> Result<(), String> {
        self.producer
            .send::<String, String>(r)
            .map_err(|(error, r)| {
                let irritant = format!("{:?}", r);
                format!("Could not send to Kafka, error={error}, irritant={irritant:.200}")
            })
    }
}

#[cfg(debug_assertions)]
struct StdoutSender {
    fail_odd_messages: bool,
}

#[cfg(debug_assertions)]
impl StdoutSender {
    fn new() -> StdoutSender {
        StdoutSender {
            fail_odd_messages: std::env::var("SONARTEST_MOCK_KAFKA")
                == Ok("fail-all-odd-messages".to_string()),
        }
    }
}

#[cfg(debug_assertions)]
impl SenderAdapter for StdoutSender {
    fn send(&self, r: BaseRecord<String, String, usize>) -> Result<(), String> {
        if self.fail_odd_messages && (r.delivery_opaque & 1) == 1 {
            println!(
                "{{\"id\":{}, \"sent\":{}, \"error\":\"Failing record\"}}",
                r.delivery_opaque,
                unix_now(),
            );
            return Err("Synthetic failure".to_string());
        }
        println!(
            "{{\"id\":{}, \"sent\":{}, \"topic\":\"{}\", \"key\":\"{}\", \"value\":{}}}",
            r.delivery_opaque,
            unix_now(),
            r.topic,
            r.key.unwrap(),
            r.payload.unwrap(),
        );
        Ok(())
    }
}

#[cfg(debug_assertions)]
fn make_sender_adapter(
    cfg: ClientConfig,
    control_and_errors: channel::Sender<Operation>,
) -> Result<Box<dyn SenderAdapter>, String> {
    if std::env::var("SONARTEST_MOCK_KAFKA").is_ok() {
        Ok(Box::new(StdoutSender::new()))
    } else {
        Ok(Box::new(KafkaSender::new(cfg, control_and_errors.clone())?))
    }
}

#[cfg(not(debug_assertions))]
fn make_sender_adapter(
    cfg: ClientConfig,
    control_and_errors: channel::Sender<Operation>,
) -> Result<Box<dyn SenderAdapter>, String> {
    Ok(Box::new(KafkaSender::new(cfg, control_and_errors.clone())?))
}

// Http REST API sender.  We use our own http uploader subsystem to do the actual pushing of bits.

fn kafka_http_producer(
    cutoff: Option<usize>,
    curl_cmd: &str,
    api_endpoint: &str,
    http_proxy: &str,
    client_id: &str,
    sasl_identity: Option<(String, String)>,
    sending_window: u64,
    timeout: u64,
    incoming_message_queue: channel::Receiver<Message<KafkaMsg>>,
    control_and_errors: channel::Sender<Operation>,
) {
    let uploader = http_upload::HttpUploader::new(curl_cmd, http_proxy, timeout);
    let op = KafkaHttpBackgroundProducer {
        uploader,
        cutoff,
        sending_window,
        client_id,
        api_endpoint,
        sasl_identity: &sasl_identity,
        control_and_errors,
    };
    background_producer(incoming_message_queue, &op);
}

struct KafkaHttpBackgroundProducer<'a> {
    uploader: http_upload::HttpUploader<'a>,
    cutoff: Option<usize>,
    sending_window: u64,
    client_id: &'a str,
    api_endpoint: &'a str,
    sasl_identity: &'a Option<(String, String)>,
    control_and_errors: channel::Sender<Operation>,
}

impl<'a> BackgroundSender<KafkaMsg> for KafkaHttpBackgroundProducer<'a> {
    fn send_all(&self, _id: usize, backlog: Vec<KafkaMsg>) {
        match self
            .uploader
            .start(self.api_endpoint, "application/octet-stream", &None)
        {
            Ok(stream) => {
                let cred = if let Some((user, pass)) = &self.sasl_identity {
                    format!("\"sasl-user\":\"{user}\", \"sasl-password\":\"{pass}\",")
                } else {
                    "".to_string()
                };
                let client = self.client_id.to_string();
                for KafkaMsg { topic, key, value } in backlog {
                    let data_size = value.len();
                    let ctrl = format!(
                        "\n{{\"topic\":\"{topic}\",\"key\":\"{key}\",\"client\":\"{client}\",{cred}\"data-size\":{data_size}}}\n"
                    );
                    stream.put_string(ctrl);
                    stream.put_string(value);
                }
                // This catches synchronous errors.  For async errors we're going to need a callback.
                // Possibly the callback is a parameter to start().
                match stream.end() {
                    Ok(()) => {}
                    Err(e) => {
                        let _ = self
                            .control_and_errors
                            .send(Operation::MessageDeliveryError(e));
                    }
                }
            }
            Err(e) => {
                // Not found, permission denied, etc.
                let _ = self.control_and_errors.send(Operation::Fatal(format!(
                    "Failed to start uploader: {:?}",
                    e
                )));
            }
        }
    }

    fn sending_window_s(&self) -> u64 {
        self.sending_window
    }

    fn shutdown_delay_ms(&self) -> u64 {
        5000
    }

    fn batch_size(&self) -> Option<usize> {
        self.cutoff
    }

    fn metadata_size(&self) -> (usize, usize) {
        // Conservative overhead for punctuation, field names, etc of the control object, note topic
        // and key have already been accounted for by the size() method, but other fields are
        // surprisingly large.
        (10 /* per batch */, 150 /* per message */)
    }
}
