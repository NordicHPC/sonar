// Implementation of KafkaManager for the kafka-rust (https://crates.io/crates/kafka) library.
//
// TODO low priority:
// - more sophisticated error handling in decode_error()
// - Shutting down connections in stop() - fairly involved, and is it worth it?
// - handle control messages (low pri b/c control messages are not useful at this time)
//   - Are there local storage needs on node for the offset store? (.with_offset_storage looks
//     scary) This comes into play for the consumer.  We need to ensure that we know what it means
//     for the backends to send messaged to clients via kafka.  They will tend to linger, even if
//     the clients only consume the latest there may be some interesting effects?  I've seen some
//     mention in the doc about storing these data on the broker, is that a thing?
//   - Testing with multiple clients.  One thing is that they should all be able to connect to the
//     broker, another is that when the broker sends a control message it should be seen by all
//     clients with that <cluster>.control.<role> combination - like a broadcast.
//
// APPARENT FACTS ABOUT THE KAFKA LIBRARY:
//
// - Connections are reopened transparently if they time out, looks like.
//
// - Connections to hosts are private to the producer/consumer; there's some reuse but we're not
//   going to benefit from that, much.  Our big efficiency gain would be from batching, if messages
//   can be batched.
//
// - send() is synchronous and will not queue up outgoing messages.
//
// - The consumer object needs to be polled to go look for messages.  When it does this it may hang
//   for a bit because it sends a message to the broker.  We can control this with
//   fetch_max_wait_time but it's unclear whether that applies only to the broker or if it is some
//   sort of network timeout control also - it mostly looks like the former.
//
// - We can choose whether to synchronously poll for control messages after sending a message, or
//   whether to do so only after sending a message *and* some time has elapsed, or whether to do so
//   regularly independently of sending the message.  In practice the computers running sonar will
//   be on continously and there's no great shame in polling once every minute or so, say (could be
//   a config control).  Or a combination.
//
// - There is *no* support for authentication.  There are several open tickets on it, but no
//   movement, it likely won't happen unless we do it ourselves.  We need SASL-PLAIN support
//   probably. This can likely be hooked into the SecurityContext but there will be some programming
//   involved.
//
// For now, we run an independent thread for the consumer, which will poll regularly (configurably)
// for control messages.

#![allow(clippy::comparison_to_empty)]

use crate::daemon::{Dur, Ini, Operation};
use crate::datasink::DataSink;
use crate::log;
use crate::time;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use kafka::{client, consumer, error, producer};
#[allow(unused_imports)]
use openssl::ssl::{SslConnector, SslFiletype, SslMethod, SslVerifyMode};

struct Message {
    timestamp: u64,
    topic: String,
    key: String,
    value: String,
}

pub struct KafkaRust {
    outgoing: Option<mpsc::Sender<Message>>,
}

// `sender` is the channel on which incoming messages from the broker will be posted will be
// posted as an Operation::Incoming(key,value).  No other messages will be posted on that
// channel from the Kafka subsystem at the moment.

impl KafkaRust {
    pub fn new(
        ini: &Ini,
        client_id: String,
        control_topic: String,
        sender: mpsc::Sender<Operation>,
    ) -> KafkaRust {
        let kafka = &ini.kafka;
        let debug = &ini.debug;

        let (kafka_sender, kafka_receiver) = mpsc::channel();

        // TODO: Here we should test-read the TLS files if they are defined and return
        // with a sane error if they can't be read, since later there's not going to be much
        // to do but panic if they disappear.

        /*

        // TODO: TLS

        let connector =
            if let (Some(cert_file), Some(key_file), Some(ca_file)) =
                (&kafka.cert_file, &kafka.key_file, &kafka.ca_file)
            {
                // TODO: Way too many unwraps
                let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
                builder.set_cipher_list("DEFAULT").unwrap();
                builder.set_verify(SslVerifyMode::PEER);
                builder
                    .set_certificate_file(cert_file, SslFiletype::PEM)
                    .unwrap();
                builder
                    .set_private_key_file(key_file, SslFiletype::PEM)
                    .unwrap();
                builder.check_private_key().unwrap();
                builder.set_ca_file(ca_file).unwrap();
                Some(builder.build())
            } else {
                None
            };
        */

        // Spawn threads to manage the Kafka connection.  This will always succeed, even if the
        // connection is not necessarily made right away.

        {
            let host = kafka.broker_address.clone();
            let verbose = debug.verbose;
            let sender = sender.clone();
            let compression = if let Some(s) = &kafka.compression {
                match s.as_ref() {
                    "gzip" => client::Compression::GZIP,
                    "snappy" => client::Compression::SNAPPY,
                    _ => client::Compression::NONE,
                }
            } else {
                client::Compression::NONE
            };
            let sending_window = ini.kafka.sending_window.to_seconds();
            thread::spawn(move || {
                kafka_producer(
                    kafka_receiver,
                    client_id,
                    sending_window,
                    compression,
                    sender,
                    host,
                    verbose,
                );
            });
        }

        {
            let host = kafka.broker_address.clone();
            let verbose = debug.verbose;
            let poll_interval = kafka.poll_interval;
            thread::spawn(move || {
                kafka_consumer(sender, host, verbose, control_topic, poll_interval);
            });
        }
        KafkaRust {
            outgoing: Some(kafka_sender),
        }
    }
}

impl DataSink for KafkaRust {
    fn post(&self, topic: String, key: String, value: String) {
        // The send can really only fail if the Kafka producer thread has closed the channel, and
        // that will only happen if it has been told to exit, so in that case ignore the error here.
        let _ignored = self.outgoing.as_ref().unwrap().send(Message{
            timestamp: time::unix_now(),
            topic,
            key,
            value,
        });
    }

    fn stop(&self) {
        // TODO: stopping things?
        //
        // - The Producer and Consumer objects held locally in the threads have their own
        //   KafkaClient instance which has a private connection pool, which holds a bunch of
        //   std::net::TcpStream objects.  When the pool is killed the streams are dropped and will
        //   be closed (according to doc).  The way to close the networks properly is therefore
        //   to exit those two threads.
        //
        // - The problem is that to do that, anything that's blocking in the two threads must be
        //   unblocked.
        //
        // - For the producer thread proper this is not too hard: close the mpsc it is listening on
        //   and it should wake up with an Err() and can exit.  For the producer thread connector
        //   part we need a flag that can be checked.  That flag must be set here.
        //
        // - For the consumer thread proper we can check the flag after the poll timeout.  But that
        //   may be quite long - typically something like many minutes.  (This is another reason
        //   why this is not a good API to be using.)  So that's not great.  For the connector part
        //   it's easier, we check every time through the loop.
        //
        // Unfortunately there's no "close" method on the queue, presumably it is closed when it
        // goes out of scope.  That sucks.  Plus it's cloned to be sent across thread boundaries so
        // there's no single point of it left.  So that means we need some kind of in-band
        // signalling.  So it's not (String,String) but some enum.
        //
        // Another question is whether we should wait to join the exiting threads, or just fire and forget.
        // Neither is a good option.
    }
}

// Here, recoverable errors are handled with maybe a log message, but fatal errors must be posted
// back to the main thread as Operation::Fatal messages.  Some errors may be transient for a while
// but then become fatal (eg authentication or authorization errors - how long do we bother to keep
// trying?)

fn kafka_producer(
    receiver: mpsc::Receiver<Message>,
    client_id: String,
    sending_window: u64,
    compression: client::Compression,
    sender: mpsc::Sender<Operation>,
    host: String,
    verbose: bool,
) {
    // From reading the kafka-rust code, creating the producer can fail in a transient way: part of
    // creation is loading metadata from the broker, and the broker may be unreachable temporarily.
    // Metadata is always loaded when creating the first client.
    //
    // To deal with that, try to loop here with a timeout trying to connect to the broker.
    let mut producer = loop {
        match producer::Producer::from_hosts(vec![host.to_string()])
            .with_ack_timeout(Duration::from_secs(1))
            .with_required_acks(producer::RequiredAcks::One)
            .with_client_id(client_id.clone())
            .with_compression(compression)
            .create()
        {
            Ok(p) => {
                if verbose {
                    log::info("Success creating producer");
                }
                break p;
            }
            Err(e) => {
                if verbose {
                    log::info(&format!("Failed to create producer, sleeping\n{e}"));
                }
                thread::sleep(Duration::from_secs(60));
            }
        }
    };

    // According to the library specs, send() is synchronous and if it returns successfully, the
    // message has been persisted on the broker.  (If we have multiple messages to send we can use
    // send_all(), but this may be somewhat unusual in our case - depends on sampling cadence
    // relative to the sending window.)
    //
    // The flip side of that is that we must maintain our own queue of outgoing messages here.  Once
    // a send() succeeds we can pop the front of the queue.  New messages go into the back of the
    // queue and are never sent until those in front of it are sent.
    //
    // We need sending to happen at random times within the sending window, to avoid creating
    // network storms on the cluster when sampling is synchronized by time.
    //
    // We want the queue to have a maximum length, which we might measure in time rather than in
    // bytes or objects.  If a message remains unsent for too long - the queue fills up behind it -
    // we might drop it, or we might drop any new messages that come in.
    //
    // To implement all this, there's a double loop. At the outermost level, we wait for data to
    // send and when there is something we move that first item into "the box".  We then wait a
    // random time within the sending window.  Then we enter the second loop: first try to send the
    // item in the box, and if it fails, then either wait for a shortish time and retry, or if the
    // item is too old, drop it on the floor.  Either way the item in the box is now discarded.  If
    // there's another item in the queue, move it into the box and re-enter the second loop.
    // Otherwise, exit to the first loop.  This way, once we start sending we send all that are
    // available.

    let mut rng = Rng::new();
    'producer_loop:
    loop {
        if verbose {
            log::info("Waiting for stuff to send");
        }
        match receiver.recv() {
            Err(_) => {
                // Channel was closed, so exit.
                break 'producer_loop;
            }
            Ok(mut msg) => {
                if sending_window > 1 {
                    let sleep = rng.next() as u64 % sending_window;
                    if verbose {
                        log::info(&format!("Sleeping {sleep} before sending"));
                    }
                    thread::sleep(Duration::from_secs(sleep));
                }
                'retry_loop:
                loop {
                    if verbose {
                        log::info(&format!("Sending to topic: {}", msg.topic));
                    }
                    match send_message(&mut producer, &msg, verbose) {
                        SendAction::Retry => {
                            if verbose {
                                log::info(&format!("Failed to send to topic {}, will retry in 1m", msg.topic));
                            }
                            thread::sleep(Duration::from_secs(60));
                            continue 'retry_loop;
                        }
                        SendAction::Sent => {
                            if verbose {
                                log::info(&format!("Sent successfully to topic: {}", msg.topic));
                            }
                            // Fall through to try_recv
                        }
                        SendAction::Timeout => {
                            if verbose {
                                log::info(&format!("Message to topic {} expired", msg.topic));
                            }
                            // Fall through to try_recv
                        }
                        SendAction::Reject(s) => {
                            if verbose {
                                log::info(&format!("Message to topic {} rejected as not sendable: {s}", msg.topic));
                            }
                            // Fall through to try_recv
                        }
                        SendAction::Fatal(s) => {
                            // Unrecoverable error, so exit.
                            if verbose {
                                log::info(&format!("Unrecoverable error sending to topic {}: {s}", msg.topic));
                            }
                            let _ = sender.send(Operation::Fatal(s));
                            break 'producer_loop;
                        }
                    }
                    match receiver.try_recv() {
                        Ok(new_msg) => {
                            msg = new_msg
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            break 'retry_loop
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            // Channel was closed, so exit
                            break 'producer_loop;
                        }
                    }
                }
            }
        }
    }
}

// There are myriad errors that can occur when trying to send.  These broadly fall into three
// groups: failure to format the message into a kafka packet, failure to send the packet, and a
// rejection of the packet by the server.  Each of these may be recoverable, nonrecoverable, or
// fatal.  The bar for fatal errors must be high - really only programming errors or non-transient
// client configuration errors.

enum SendAction {
    Sent,                       // Message was sent successfully
    Retry,                      // Retry in a bit
    Timeout,                    // Discard b/c the message expired
    Reject(String),             // Discard b/c the message can't ever be sent
    Fatal(String),              // Client is in unrecoverable state relative to server, abort
}

fn send_message(producer: &mut producer::Producer, msg: &Message, verbose: bool) -> SendAction {
    match producer.send(&producer::Record::from_key_value(
        &msg.topic,
        msg.key.as_bytes(),
        msg.value.as_bytes(),
    )) {
        Ok(_) => SendAction::Sent,
        Err(e) => {
            if verbose {
                log::info(&format!("Sending failure: {e}"));
            }
            decode_error(msg, e)
        }
    }
}

// TODO: handle error cases more deeply, these are just some examples.

fn decode_error(msg: &Message, e: error::Error) -> SendAction {
    match e {
        error::Error::NoHostReachable => SendAction::Retry,
        error::Error::Io(_) => timeout_or_retry(msg),
        error::Error::CodecError | error::Error::StringDecodeError =>
            SendAction::Reject(format!("{e}")),
        error::Error::InvalidSnappy(e) =>
            SendAction::Fatal(format!("{e}")),
        _ => SendAction::Retry
    }
}

fn timeout_or_retry(msg: &Message) -> SendAction {
    if time::unix_now() - msg.timestamp > 30*60 {
        SendAction::Timeout
    } else {
        SendAction::Retry
    }
}

// Recoverable errors are handled with a log message, but fatal errors must be posted back to the
// main thread as Operation::Fatal messages.

fn kafka_consumer(
    sender: mpsc::Sender<Operation>,
    host: String,
    verbose: bool,
    control_topic: String,
    poll_interval: Dur,
) {
    // No group as of yet - don't know if we need it, don't know what the implications are.  We need
    // to not risk needing local state on the node.
    let group = "";

    // See comments above about the producer, we need to be resilient to transient errors when
    // creating the consumer.
    let mut consumer = loop {
        match consumer::Consumer::from_hosts(vec![host.clone()])
            .with_topic(control_topic.clone())
            .with_fallback_offset(consumer::FetchOffset::Latest)
            .with_group(group.to_string())
            .create()
        {
            Ok(c) => {
                if verbose {
                    log::info(&format!("Success creating consumer of {control_topic}"));
                }
                break c;
            }
            Err(e) => {
                if verbose {
                    log::info(&format!("Failed to create consumer of {control_topic}\nReason: {e}\nSleeping 1m"));
                }
                thread::sleep(Duration::from_secs(60));
            }
        }
    };

    'consumer_loop: loop {
        thread::sleep(std::time::Duration::from_secs(poll_interval.to_seconds()));
        let responses = match consumer.poll() {
            Ok(r) => r,
            Err(e) => {
                // This happens at least for "unknown topic or partition".  That can be a transient
                // or permanent error.  As we'll be polling at a limited rate it's OK to just log
                // the problem and try again later.
                if verbose {
                    log::info(&format!("Consumer error: {e}"));
                }
                continue;
            }
        };
        for ms in responses.iter() {
            for m in ms.messages() {
                let key = String::from_utf8_lossy(m.key).to_string();
                let value = String::from_utf8_lossy(m.value).to_string();
                match sender.send(Operation::Incoming(key, value)) {
                    Ok(_) => {}
                    Err(e) => {
                        // Channel was closed, no option but to exit.
                        if verbose {
                            log::info(&format!("Send error on consumer channel: {e}"));
                        }
                        break 'consumer_loop;
                    }
                }
            }
            match consumer.consume_messageset(ms) {
                Ok(_) => {}
                Err(e) => {
                    if verbose {
                        log::info(&format!("Could not consume: {e}"));
                    }
                }
            }
        }
        if group != "" {
            match consumer.commit_consumed() {
                Ok(()) => {}
                Err(e) => {
                    if verbose {
                        log::info(&format!("Could not commit consumed: {e}"));
                    }
                }
            }
        }
    }
}

// Generate randomish u32 numbers

pub struct Rng {
    state: u32                  // nonzero
}

impl Rng {
    pub fn new() -> Rng {
        Rng { state: crate::time::unix_now() as u32 }
    }

    // https://en.wikipedia.org/wiki/Xorshift, this supposedly has period 2^32-1 but is not "very
    // random".
    pub fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[test]
pub fn rng_test() {
    let mut r = Rng::new();
    let a = r.next();
    let b = r.next();
    let c = r.next();
    let d = r.next();
    // It's completely unlikely that they're all equal, so that would indicate some kind of bug.
    assert!(!(a == b && b == c && c == d));
}
