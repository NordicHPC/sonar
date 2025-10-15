// Implementation of DataSink for the rdkafka (https://crates.io/crates/rdkafka) library.
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
// See TODOs throughout for minor issues around error handling, especially.

#![allow(clippy::comparison_to_empty)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use crate::daemon::{Dur, Ini, Operation};
use crate::datasink::DataSink;
use crate::log;
use crate::systemapi::SystemAPI;
#[cfg(debug_assertions)]
use crate::time::unix_now;
use crate::util;

use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel;

use rdkafka::client::ClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::message::DeliveryResult;
use rdkafka::producer::base_producer::ThreadedProducer;
use rdkafka::producer::{BaseProducer, BaseRecord, NoCustomPartitioner, ProducerContext};

struct Msg {
    timestamp: u64,
    topic: String,
    key: String,
    value: String,
}

enum Message {
    Stop,
    M(Msg),
}

pub struct RdKafka {
    outgoing_message_queue: channel::Sender<Message>,
    producer: Option<thread::JoinHandle<()>>,
}

// `control_and_errors` is the channel on which incoming control messages from the broker and any
// errors will be posted (as Operation::Incoming(key,value), Operation::MessageDeliveryError(msg),
// or Operation::Fatal(msg), respectively).

impl RdKafka {
    pub fn new(
        ini: &Ini,
        client_id: String,
        control_topic: String,
        control_and_errors: channel::Sender<Operation>,
    ) -> RdKafka {
        let global = &ini.global;
        let kafka = &ini.kafka;
        let debug = &ini.debug;
        let (outgoing_message_queue, incoming_message_queue) = channel::unbounded();
        let producer = {
            let broker = kafka.broker_address.clone();
            let ca_file = kafka.ca_file.clone();
            let sasl_identity = kafka
                .sasl_password
                .as_ref()
                .map(|password| (global.cluster.clone(), password.clone()));
            let sending_window = ini.kafka.sending_window.to_seconds();
            let timeout = ini.kafka.timeout.to_seconds();
            let verbose = debug.verbose;
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
                    verbose,
                );
            })
        };

        RdKafka {
            outgoing_message_queue,
            producer: Some(producer),
        }
    }
}

impl DataSink for RdKafka {
    fn post(
        &mut self,
        system: &dyn SystemAPI,
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
        let _ignored = self.outgoing_message_queue.send(Message::M(Msg {
            timestamp: system.get_now_in_secs_since_epoch(),
            topic,
            key,
            value,
        }));
    }

    fn stop(&mut self, _system: &dyn SystemAPI) {
        let _ = self.outgoing_message_queue.send(Message::Stop);
        let mut producer = None;
        std::mem::swap(&mut producer, &mut self.producer);
        let _ = producer.unwrap().join();
    }
}

// Sending logic works like this:
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
    verbose: bool,
    control_and_errors: channel::Sender<Operation>,
}

impl ClientContext for SonarProducerContext {}

const KAFKA_BUFFER_MS: usize = 1000;

fn kafka_producer(
    broker: String,
    client_id: String,
    ca_file: Option<String>,
    sasl_identity: Option<(String, String)>,
    sending_window: u64,
    timeout: u64,
    incoming_message_queue: channel::Receiver<Message>,
    control_and_errors: channel::Sender<Operation>,
    verbose: bool,
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
    let producer = make_sender_adapter(cfg, control_and_errors.clone(), verbose)
        .expect("Producer creation error");

    let mut id = 0usize;
    let mut rng = util::Rng::new();
    let mut timeout: channel::Receiver<Instant> = channel::never();
    let mut armed = false;
    let mut must_arm = false;
    let mut backlog = Vec::new();

    'producer_loop: loop {
        if must_arm {
            assert!(!armed);
            let sleep = rng.next() as u64 % sending_window;
            if verbose {
                // Note, the /Sleeping {} before sending/ pattern is used by regression tests.
                log::verbose(&format!("Sleeping {sleep} before sending"));
            }
            timeout = channel::after(Duration::from_secs(sleep));
            armed = true;
            must_arm = false;
        }
        channel::select! {
            recv(timeout) -> _ => {
                armed = false;
                if verbose {
                    // Note, the /Sending {} items/ pattern is used by regression tests.
                    log::verbose(&format!("Sending window open.  Sending {} items", backlog.len()));
                }
                id = send_messages(&*producer, &control_and_errors, id, &backlog, verbose);
                backlog.clear();
            }
            recv(incoming_message_queue) -> msg => match msg {
                Ok(Message::M(msg)) => {
                    backlog.push(msg);
                    must_arm = !armed;
                }
                Ok(Message::Stop) | Err(_) => {
                    _ = send_messages(&*producer, &control_and_errors, id, &backlog, verbose);
                    break 'producer_loop;
                }
            }
        }
    }

    // Best effort: give the Kafka thread an opportunity to send what it has.
    thread::sleep(Duration::from_millis(2 * KAFKA_BUFFER_MS as u64));
}

fn send_messages(
    producer: &dyn SenderAdapter,
    control_and_errors: &channel::Sender<Operation>,
    mut id: usize,
    backlog: &[Msg],
    verbose: bool,
) -> usize {
    // We always try to send everything.  Messages that fail are dropped, because the only failure
    // is failure to be enqueued - in that case, the message is probably fatally flawed.
    for msg in backlog.iter() {
        id += 1; // Always give it a new ID, even if it is later dropped.
        if verbose {
            log::verbose(&format!("Sending to topic: {} with id {id}", msg.topic));
        }
        match producer.send(
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
                let _ = control_and_errors.send(Operation::MessageDeliveryError(msg));
            }
        }
    }
    id
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
                if self.verbose {
                    log::verbose(&format!("Sent #{delivery_opaque} successfully"));
                }
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
        verbose: bool,
    ) -> Result<KafkaSender, String> {
        let producer: ThreadedProducer<SonarProducerContext, NoCustomPartitioner> =
            cfg
                .create_with_context::<SonarProducerContext,
                                       ThreadedProducer<SonarProducerContext, NoCustomPartitioner>>(
                    SonarProducerContext { verbose, control_and_errors },
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
    verbose: bool,
) -> Result<Box<dyn SenderAdapter>, String> {
    if std::env::var("SONARTEST_MOCK_KAFKA").is_ok() {
        Ok(Box::new(StdoutSender::new()))
    } else {
        Ok(Box::new(KafkaSender::new(
            cfg,
            control_and_errors.clone(),
            verbose,
        )?))
    }
}

#[cfg(not(debug_assertions))]
fn make_sender_adapter(
    cfg: ClientConfig,
    control_and_errors: channel::Sender<Operation>,
    verbose: bool,
) -> Result<Box<dyn SenderAdapter>, String> {
    Ok(Box::new(KafkaSender::new(
        cfg,
        control_and_errors.clone(),
        verbose,
    )?))
}
