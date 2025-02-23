// Implementation of KafkaManager for the kafka-rust (https://crates.io/crates/kafka) library.
//
// TODO features in decreasing priority order
//  - tls
//  - authentication (want broker to control posting to data topics)
//    - can't have anything real at the moment, would have to fake it
//    - without tls we'll need to have bespoke symmetric crypto, ideally one key per cluster
//      which means cluster identity has to be external to encrypted data, should be OK
//      since that's part of the topic
//  - randomized sending time within a window
//  - control pileup of unsent messages
//
// TODO other:
// - Shutting down connections in stop() - fairly involved, and is it worth it?
// - better error handling?
// - Are there local storage needs on node for the offset store? (.with_offset_storage looks scary)
//   This comes into play for the consumer.  We need to ensure that we know what it means for
//   the backends to send messaged to clients via kafka.  They will tend to linger, even if
//   the clients only consume the latest there may be some interesting effects?  I've seen some
//   mention in the doc about storing these data on the broker, is that a thing?
// - Testing with multiple clients.  One thing is that they should all be able to connect to the
//   broker, another is that when the broker sends a control message it should be seen by all
//   clients with that <cluster>.control.<role> combination - like a broadcast.
//
// APPARENT FACTS ABOUT THE KAFKA LIBRARY:
//
// - Connections are reopened transparently if they time out, looks like.
//
// - Connections to hosts are private to the producer/consumer; there's some reuse but we're not
//   going to benefit from that, much.  Our big efficiency gain would be from batching, if messages
//   can be batched.
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
// For now, we run an independent thread for the consumer, which will poll regularly (configurably)
// for control messages.
//
// - There is *no* support for authentication.  There are several open tickets on it, but no
//   movement, it likely won't happen unless we do it ourselves.  We need SASL-PLAIN support
//   probably. This can likely be hooked into the SecurityContext but there will be some programming
//   involved.

use crate::daemon::{Dur, Ini, KafkaManager, Operation};
use crate::log;

use std::fs;
use std::path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use kafka::{client, consumer, producer};
#[allow(unused_imports)]
use openssl::ssl::{SslConnector, SslFiletype, SslMethod, SslVerifyMode};

pub struct KafkaKafka<K, V> {
    outgoing: Option<mpsc::Sender<(String, K, V)>>,
}

// `sender` is the channel on which incoming messages from the broker will be posted will be
// posted as an Operation::Incoming(key,value).  No other messages will be posted on that
// channel from the Kafka subsystem at the moment.

pub fn new_kafka<K, V>() -> KafkaKafka<K, V> {
    KafkaKafka::<K, V> { outgoing: None }
}

impl<
        K: std::marker::Send + producer::AsBytes + 'static,
        V: std::marker::Send + producer::AsBytes + 'static,
    > KafkaManager<K, V> for KafkaKafka<K, V>
{
    fn init(
        &mut self,
        ini: &Ini,
        client_id: String,
        sender: mpsc::Sender<Operation>,
    ) -> Result<(), String> {
        let global = &ini.global;
        let kafka = &ini.kafka;
        let debug = &ini.debug;

        if self.outgoing.is_some() {
            return Err("Already initialized".to_string());
        }

        let (kafka_sender, kafka_receiver) = mpsc::channel();

        let _password = if let Some(filename) = &kafka.password_file {
            match fs::read_to_string(path::Path::new(filename)) {
                Ok(s) => s,
                Err(_) => {
                    return Err("Can't read password file".to_string());
                }
            }
        } else {
            "".to_string()
        };

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
            thread::spawn(move || {
                kafka_producer(kafka_receiver, client_id, compression, sender, host, verbose);
            });
        }

        {
            let host = kafka.broker_address.clone();
            let verbose = debug.verbose;
            let control_topic = global.cluster.clone() + ".control." + &global.role;
            let poll_interval = kafka.poll_interval;
            thread::spawn(move || {
                kafka_consumer(sender, host, verbose, control_topic, poll_interval);
            });
        }

        self.outgoing = Some(kafka_sender);
        Ok(())
    }

    fn post(&self, topic: String, key: K, value: V, _sending_window: u64) {
        // The send can really only fail if the Kafka producer thread has closed the channel, and
        // that will only happen if it has been told to exit, so in that case ignore the error here.
        //
        // TODO: prevent unsent messages from piling up somehow?
        //
        // TODO: Deal with sending_window.  If 0, messages are to be sent asap.  Otherwise, they are
        // to be held for a random number of seconds between now and now+sending_window, but
        // to still be sent in order.
        let _ignored = self.outgoing.as_ref().unwrap().send((topic, key, value));
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

fn kafka_producer<K: producer::AsBytes, V: producer::AsBytes>(
    receiver: mpsc::Receiver<(String, K, V)>,
    client_id: String,
    compression: client::Compression,
    _sender: mpsc::Sender<Operation>,
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
                // TODO: Check a flag here
            }
        }
    };

    loop {
        if verbose {
            log::info("Waiting for stuff to send");
        }
        match receiver.recv() {
            Ok((topic, key, value)) => {
                if verbose {
                    log::info(&format!("Sending it: {topic}"));
                }
                match producer.send(&producer::Record::from_key_value(
                    &topic,
                    key.as_bytes(),
                    value.as_bytes(),
                )) {
                    Ok(_) => {}
                    Err(e) => {
                        // TODO: Unclear what to do here.  What are the error conditions?  Will this
                        // happen if a connection goes down?  If so, do we re-enqueue the message
                        // (probably we have a private queue for failed messages) and hope for the
                        // best?  Or does the library take care of this?
                        if verbose {
                            log::info(&format!("Could not send to Kafka: {e}"));
                        }
                    }
                }
            }
            Err(_) => {
                // Channel was closed, so exit.
                break;
            }
        }
    }
}

// Here, recoverable errors are handled with maybe a log message, but fatal errors must be posted
// back to the main thread as Operation::Fatal messages.

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
                break c;
            }
            Err(e) => {
                if verbose {
                    log::info(&format!("Failed to create consumer, sleeping\n{e}"));
                }
                thread::sleep(Duration::from_secs(60));
                // TODO: check a flag here and exit if set
            }
        }
    };

    'consumer_loop: loop {
        thread::sleep(std::time::Duration::from_secs(poll_interval.to_seconds()));
        // TODO: check a flag here and exit if set
        let responses = match consumer.poll() {
            Ok(r) => r,
            Err(e) => {
                // This happens at least for "unknown topic or partition".  That can be a transient
                // or permanent error.  Not sure what to do, but since we'll be polling at a limited
                // rate it's OK to just log the problem and try again later.
                log::info(&format!("Consumer error: {e}"));
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
                        // Channel was closed, no option but to exit?
                        log::info(&format!("Send error on consumer channel: {e}"));
                        break 'consumer_loop;
                    }
                }
            }
            match consumer.consume_messageset(ms) {
                Ok(_) => {}
                Err(e) => {
                    log::info(&format!("Could not consume: {e}"));
                }
            }
        }
        if group != "" {
            match consumer.commit_consumed() {
                Ok(()) => {}
                Err(e) => {
                    log::info(&format!("Could not commit consumed: {e}"));
                }
            }
        }
    }
}
