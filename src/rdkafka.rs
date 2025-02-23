// INCOMPLETE.  Implementation of KafkaManager for the rdkafka library.

use crate::daemon::{DebugIni, Dur, GlobalIni, KafkaIni, KafkaManager, Operation};

use std::sync::mpsc;

// use rdkafka::config::ClientConfig;
// use rdkafka::message::{Header, OwnedHeaders};
// use rdkafka::producer::{FutureProducer, BaseProducer, FutureRecord};

pub struct RdKafka {
    outgoing: Option<mpsc::Sender<(String, String)>>,
}

// `sender` is the channel on which incoming messages from the broker will be posted will be
// posted as an Operation::Incoming(key,value).  No other messages will be posted on that
// channel from the Kafka subsystem at the moment.

pub fn new_kafka() -> RdKafka {
    RdKafka { outgoing: None }
}

impl KafkaManager for RdKafka {
    fn init(
        &mut self,
        global: &GlobalIni,
        kafka: &KafkaIni,
        debug: &DebugIni,
        sender: mpsc::Sender<Operation>,
    ) -> Result<(), String> {
        if self.outgoing.is_some() {
            return Err("Already initialized".to_string());
        }

        let (kafka_sender, kafka_receiver) = mpsc::channel();

        {
            let host = kafka.remote_host.clone();
            let verbose = debug.verbose;
            thread::spawn(move || {
                kafka_producer(kafka_receiver, host, verbose);
            });
        }

        {
            let host = kafka.remote_host.clone();
            let verbose = debug.verbose;
            let control_topic = global.cluster.clone() + "." + &global.role;
            let poll_interval = kafka.poll_interval;
            thread::spawn(move || {
                kafka_consumer(sender, host, verbose, control_topic, poll_interval);
            });
        }

        self.outgoing = Some(kafka_sender);
        Ok(())
    }

    fn post(&self, _topic: String, _body: String) {
        let _ignored = self.outgoing.as_ref().unwrap().send((topic, body));
    }

    fn stop(&self) {
        todo!()
    }
}

fn kafka_producer(receiver: mpsc::Receiver<(String, String)>, host: String, verbose: bool) {
    let producer: BaseProducer = ClientConfig::new()
        .set("bootstrap.servers", &host)
        .create()
        .expect("Producer creation error");
    let mut pending = 0;
    loop {
        // Here we can have multiple input sources: the receiver queue, and the producer's incoming
        // channel (replies).  The logic would be that if a reply is pending then we will not block
        // on the receiver queue, but will do a try_recv, and if that goes nowhere, will block on
        // the incoming channel for a short time.  If a reply is not pending then we can do a
        // blocking receive on the receiver queue.  The purpose is to not poll the network unless
        // there's reason to.
        //
        // The underlying assumption is that we can know the number of replies we should get.  It
        // appears so: The ProducerContext::delivery method is always called with a payload that
        // identifies the sent message.  That method is called async, ie from some other thread.
        // The utility of the poll() is to drive the work of processing replies.
        //
        // TBD whether poll is smart enough to just return if there are no outstanding replies,
        // but it wouls still be good not to poll since it basically amounts to busy-waiting.
        match receiver.recv() {
            Ok((topic, body)) => {
                // Probably the key should be the node name?  It's redundant because it's
                // also in the record (the cluster name is in the topic), but might
                // be useful for some filtering?
                match producer.send(&BaseRecord::to(&topic).payload(body).key("")) {
                    Ok(_) => {
                        pending += 1;
                    }
                    Err(e) => {
                        // TODO: Unclear what to do here.
                        if verbose {
                            println!("Could not send to Kafka: {e}");
                        }
                    }
                }
            }
            Err(_) => {
                // Channel was closed, this indicates that we should exit cleanly
                break;
            }
        }
    }
}

fn kafka_consumer(
    _sender: mpsc::Sender<Operation>,
    _host: String,
    _verbose: bool,
    _control_topic: String,
    _poll_interval: Dur,
) {
    // FIXME
}
