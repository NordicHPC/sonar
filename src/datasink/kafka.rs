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
use crate::datasink::DataSink;
use crate::systemapi::SystemAPI;
#[cfg(debug_assertions)]
use crate::time::unix_now;
use crate::util;

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

pub struct KafkaSink {
    outgoing_message_queue: channel::Sender<Message>,
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
                let rest_endpoint = kafka.rest_endpoint.clone();
                let rest_proxy = kafka.rest_proxy.clone();
                let curl_cmd = if let Some(ref curl) = ini.programs.curl_cmd {
                    curl.clone()
                } else {
                    "curl".to_string()
                };
                thread::spawn(move || {
                    http_producer(
                        &curl_cmd,
                        &rest_endpoint,
                        &rest_proxy,
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
            // Note, the /Sleeping {} before sending/ pattern is used by regression tests.
            log::debug!("Sleeping {sleep} before sending");
            timeout = channel::after(Duration::from_secs(sleep));
            armed = true;
            must_arm = false;
        }
        channel::select! {
            recv(timeout) -> _ => {
                armed = false;
                // Note, the /Sending {} items/ pattern is used by regression tests.
                log::debug!("Sending window open.  Sending {} items", backlog.len());
                id = kafka_send_messages(&*producer, &control_and_errors, id, &backlog);
                backlog.clear();
            }
            recv(incoming_message_queue) -> msg => match msg {
                Ok(Message::M(msg)) => {
                    backlog.push(msg);
                    must_arm = !armed;
                }
                Ok(Message::Stop) | Err(_) => {
                    _ = kafka_send_messages(&*producer, &control_and_errors, id, &backlog);
                    break 'producer_loop;
                }
            }
        }
    }

    // Best effort: give the Kafka thread an opportunity to send what it has.
    thread::sleep(Duration::from_millis(2 * KAFKA_BUFFER_MS as u64));
}

fn kafka_send_messages(
    producer: &dyn SenderAdapter,
    control_and_errors: &channel::Sender<Operation>,
    mut id: usize,
    backlog: &[Msg],
) -> usize {
    // We always try to send everything.  Messages that fail are dropped, because the only failure
    // is failure to be enqueued - in that case, the message is probably fatally flawed.
    for msg in backlog.iter() {
        id += 1; // Always give it a new ID, even if it is later dropped.
        log::debug!("Sending to topic: {} with id {id}", msg.topic);
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

// Http REST API sender.  The logic here is that to send a data package we fork off a curl and make
// it send the output and handle retries, it will automatically pick up proxy settings from the
// environment.  The main thread does not wait for it to finish but spins up threads to handle its
// stdin/stdout/stderr and the final wait.

fn http_producer(
    cmd: &str,
    api_endpoint: &str,
    proxy_address: &str,
    client_id: &str,
    sasl_identity: Option<(String, String)>,
    sending_window: u64,
    mut timeout: u64,
    incoming_message_queue: channel::Receiver<Message>,
    control_and_errors: channel::Sender<Operation>,
) {
    // Curl will retry for 1s, 2s, 4s, ..., 10m and then stick to 10m
    let mut retry_count = 0;
    let mut next = 1;
    while timeout > 0 {
        timeout -= min(next, timeout);
        next = min(600, next * 2);
        retry_count += 1;
    }
    let mut rng = util::Rng::new();
    let mut timeout: channel::Receiver<Instant> = channel::never();
    let mut armed = false;
    let mut must_arm = false;
    let mut backlog = Vec::new();

    'producer_loop: loop {
        if must_arm {
            assert!(!armed);
            let sleep = rng.next() as u64 % sending_window;
            // Note, the /Sleeping {} before sending/ pattern is used by regression tests.
            log::debug!("Sleeping {sleep} before sending");
            timeout = channel::after(Duration::from_secs(sleep));
            armed = true;
            must_arm = false;
        }
        channel::select! {
            recv(timeout) -> _ => {
                armed = false;
                // Note, the /Sending {} items/ pattern is used by regression tests.
                log::debug!("Sending window open.  Sending {} items", backlog.len());
                http_send_messages(cmd, api_endpoint, proxy_address, client_id, retry_count, &sasl_identity, &control_and_errors, backlog);
                backlog = vec![];
            }
            recv(incoming_message_queue) -> msg => match msg {
                Ok(Message::M(msg)) => {
                    backlog.push(msg);
                    must_arm = !armed;
                }
                Ok(Message::Stop) | Err(_) => {
                    http_send_messages(cmd, api_endpoint, proxy_address, client_id, retry_count, &sasl_identity, &control_and_errors, backlog);
                    break 'producer_loop;
                }
            }
        }
    }

    // Best effort: give worker threads an opportunity to send what they have.
    thread::sleep(Duration::from_millis(5000));
}

// TODO: Must control message volume!  The broker limits the content-length to 1GB and we must be
// sure never to exceed that.

fn http_send_messages(
    cmd: &str,
    api_endpoint: &str,
    proxy_address: &str,
    client_id: &str,
    retry_count: i32,
    sasl_identity: &Option<(String, String)>,
    control_and_errors: &channel::Sender<Operation>,
    backlog: Vec<Msg>,
) {
    let mut args = vec![
        "--data-binary".to_string(),
        "@-".to_string(),
        "-H".to_string(),
        "Content-Type: application/octet-stream".to_string(),
    ];
    if retry_count > 0 {
        args.push("--retry".to_string());
        args.push(format!("{}", retry_count));
        args.push("--retry-connrefused".to_string());
    }
    args.push(api_endpoint.to_string());

    // Really want to merge stdout and stderr
    let mut cmd = std::process::Command::new(cmd);
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if proxy_address != "" {
        cmd.env("http_proxy", proxy_address)
            .env("https_proxy", proxy_address);
    }
    match cmd.spawn() {
        Ok(mut child) => {
            if let (Some(mut stdin), Some(mut stdout), Some(mut stderr)) =
                (child.stdin.take(), child.stdout.take(), child.stderr.take())
            {
                let cred = if let Some((user, pass)) = sasl_identity {
                    format!("\"sasl-user\":\"{user}\", \"sasl-password\":\"{pass}\",")
                } else {
                    "".to_string()
                };
                let client = client_id.to_string();
                drop(std::thread::spawn(move || {
                    for Msg {
                        timestamp,
                        topic,
                        key,
                        value,
                    } in backlog
                    {
                        let _ = timestamp;
                        let data_size = value.len();
                        let ctrl = format!(
                            "\n{{\"topic\":\"{topic}\",\"key\":\"{key}\",\"client\":\"{client}\",{cred}\"data-size\":{data_size}}}\n"
                        );
                        let _ = stdin.write_all(ctrl.as_bytes());
                        let _ = stdin.write_all(value.as_bytes());
                    }
                    drop(stdin);
                }));
                // Separate threads do the output consuming and waiting for curl to finish, in order to
                // guarantee several things:
                //
                //  - if the curl does not terminate immediately (because it is retrying, not
                //    reaching the host, bandwidth-limited, ...) then Sonar does not hang waiting
                //    for it, but can get on with its work.
                //  - curl will not block writing its output on a full pipe
                //  - sonar will not block on writing to curl because curl is blocked
                //  - the child does not linger once it's ready to exit
                //
                // We can't use wait_with_output() because that will close stdin, and we can't wait
                // to fork off these thread until writing has completed.  We could maybe combine the
                // two reader threads into one using some kind of nonblocking I/O.  Maybe there are
                // other tricks.
                drop(std::thread::spawn(move || {
                    let mut buf = [0; 1024];
                    loop {
                        match stdout.read(&mut buf[..]) {
                            Err(_) | Ok(0) => {
                                break;
                            }
                            Ok(_) => {}
                        }
                    }
                    drop(stdout);
                }));
                drop(std::thread::spawn(move || {
                    let mut buf = [0; 1024];
                    loop {
                        match stderr.read(&mut buf[..]) {
                            Err(_) | Ok(0) => {
                                break;
                            }
                            Ok(_) => {}
                        }
                    }
                    drop(stderr);
                }));
                drop(std::thread::spawn(move || {
                    let _ = child.wait();
                }));
            } else {
                // Should never happen
                let _ = control_and_errors.send(Operation::MessageDeliveryError(
                    "Failed to get stdin/stdout/stderr".to_string(),
                ));
            }
        }
        Err(e) => {
            // Not found, permission denied, etc.
            let _ = control_and_errors
                .send(Operation::Fatal(format!("Failed to launch curl: {:?}", e)));
        }
    }
}
