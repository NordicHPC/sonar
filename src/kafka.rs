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
// See TODOs throughout.

#![allow(clippy::comparison_to_empty)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use crate::daemon::{Dur, Ini, Operation};
use crate::datasink::DataSink;
use crate::log;
use crate::time;
use crate::util;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rdkafka::client::ClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::message::DeliveryResult;
use rdkafka::producer::base_producer::ThreadedProducer;
use rdkafka::producer::{BaseRecord, NoCustomPartitioner, ProducerContext};

struct Message {
    timestamp: u64,
    topic: String,
    key: String,
    value: String,
}

pub struct RdKafka {
    outgoing_message_queue: mpsc::Sender<Message>,
}

// `control_and_errors` is the channel on which incoming control messages from the broker and any
// errors will be posted (as Operation::Incoming(key,value), Operation::MessageDeliveryError(msg),
// or Operation::Fatal(msg), respectively).

impl RdKafka {
    pub fn new(
        ini: &Ini,
        client_id: String,
        control_topic: String,
        control_and_errors: mpsc::Sender<Operation>,
    ) -> RdKafka {
        let global = &ini.global;
        let kafka = &ini.kafka;
        let debug = &ini.debug;
        let (outgoing_message_queue, incoming_message_queue) = mpsc::channel();

        {
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
            });
        }

        RdKafka {
            outgoing_message_queue,
        }
    }
}

impl DataSink for RdKafka {
    fn post(&self, topic: String, key: String, value: String) {
        // The send can really only fail if the Kafka producer thread has closed the channel, and
        // that will only happen if stop() has been called, and post() should never be called after
        // stop(), so in that case ignore the error here.
        let _ignored = self.outgoing_message_queue.send(Message {
            timestamp: time::unix_now(),
            topic,
            key,
            value,
        });
    }

    fn stop(&self) {
        // Nothing happens here.  The owner of the DataSink should drop it after calling stop().
        // Eventually all clones of outgoing_message_queue are dropped and the receive in the
        // producer will error out, and the producer will exit.
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
// Batching time is set to 1s as that will give us enough time to batch everything that's queued up,
// but in normal operation the sending window will be shorter than the cadence and there will not
// be a queue.

struct SonarProducerContext {
    verbose: bool,
    control_and_errors: mpsc::Sender<Operation>,
}

impl ClientContext for SonarProducerContext {}

fn kafka_producer(
    broker: String,
    client_id: String,
    ca_file: Option<String>,
    sasl_identity: Option<(String, String)>,
    sending_window: u64,
    timeout: u64,
    incoming_message_queue: mpsc::Receiver<Message>,
    control_and_errors: mpsc::Sender<Operation>,
    verbose: bool,
) {
    let mut cfg = ClientConfig::new();
    cfg.set("bootstrap.servers", &broker)
        .set("client.id", &client_id)
        .set("queue.buffering.max.ms", "1000")
        .set("message.timeout.ms", &format!("{}", timeout * 1000))
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
    let producer: &ThreadedProducer<SonarProducerContext, NoCustomPartitioner> = {
        let control_and_errors = control_and_errors.clone();
        &cfg
            .create_with_context::<SonarProducerContext,
                                   ThreadedProducer<SonarProducerContext, NoCustomPartitioner>>(
                SonarProducerContext { verbose, control_and_errors },
            )
            .expect("Producer creation error")
    };

    let mut id = 0;
    let mut rng = util::Rng::new();
    'producer_loop: loop {
        if verbose {
            log::verbose("Waiting for stuff to send");
        }
        match incoming_message_queue.recv() {
            Err(_) => {
                // Channel was closed, so exit.
                break 'producer_loop;
            }
            Ok(mut msg) => {
                if sending_window > 1 {
                    let sleep = rng.next() as u64 % sending_window;
                    if verbose {
                        log::verbose(&format!("Sleeping {sleep} before sending"));
                    }
                    // TODO: This is really not ideal.  We should be setting up a timer and then
                    // sending the message when the timer expires.  But we won't fix this until we
                    // move from mpsc to crossbeam.
                    thread::sleep(Duration::from_secs(sleep));
                }

                'sender_loop: loop {
                    id += 1;
                    if verbose {
                        log::verbose(&format!("Sending to topic: {} with id {id}", msg.topic));
                    }
                    match producer.send(
                        BaseRecord::with_opaque_to(&msg.topic, id)
                            .payload(&msg.value)
                            .key(&msg.key),
                    ) {
                        Ok(()) => {}
                        Err((m, e)) => {
                            // TODO: There are various problems with sending here that we should
                            // maybe try to figure out and signal in a sensible way.  For now,
                            // drop the message, and go back to the slow loop, do not flush the
                            // message queue.
                            let msg = format!("Message production error {:?} {}", e, m);
                            let _ = control_and_errors.send(Operation::MessageDeliveryError(msg));
                            continue 'producer_loop;
                        }
                    }

                    // Once we're sending, send everything we've got, or we may get backed up if the
                    // production cadence is higher than the sending cadence.
                    match incoming_message_queue.try_recv() {
                        Err(mpsc::TryRecvError::Empty) => {
                            break 'sender_loop;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            break 'producer_loop;
                        }
                        Ok(m) => {
                            msg = m;
                        }
                    }
                }
            }
        }
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
                if self.verbose {
                    log::verbose(&format!("Sent #{delivery_opaque} successfully"));
                }
            }
            Err((e, m)) => {
                // TODO: The message could not be sent.  We could try to disambiguate here and try
                // different actions, but for now, just drop it on the floor.
                let msg = format!("Message production error {delivery_opaque} {:?}", e);
                let _ = self
                    .control_and_errors
                    .send(Operation::MessageDeliveryError(msg));
            }
        }
    }
}
