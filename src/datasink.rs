use crate::daemon;

use std::io;
use std::sync::mpsc;
use std::thread;

// The DataSink hides the specific data sink we use.  It receives outgoing traffic by `post()` and
// posts any incoming messages or errors on `sender`.  The sink may batch outgoing messages, and its
// network connection - if there is one - may go up and down, and so on.

pub trait DataSink {
    // Queue the message for sending, to be sent within the sending window.
    fn post(&self, topic: String, key: String, value: String);

    // Stop the sink.
    fn stop(&self);
}

// Trivial data sink.  This dumps the output as JSON on stdout, and reads command messages from
// stdin, on the form /target\s+key\s+value/.

pub struct StdioSink {
    client_id: String,
}

impl StdioSink {
    pub fn new(
        client_id: String,
        control_topic: String,
        sender: mpsc::Sender<daemon::Operation>,
    ) -> StdioSink {
        thread::spawn(move || {
            control_message_reader(control_topic, sender);
        });
        StdioSink { client_id }
    }
}

impl DataSink for StdioSink {
    fn post(&self, topic: String, key: String, value: String) {
        println!(
            "{{\"topic\":\"{topic}\",\n \"key\":\"{key}\",\n \"client\":\"{}\",\n \"value\":{value}}}",
            self.client_id,
        );
    }

    fn stop(&self) {
        // TODO: This (maybe) needs to kill the stdin thread
    }
}

fn control_message_reader(control_topic: String, sender: mpsc::Sender<daemon::Operation>) {
    for line in io::stdin().lines() {
        match line {
            Ok(s) => {
                let mut fields = s.split_ascii_whitespace();
                if let Some(topic) = fields.next() {
                    if control_topic == topic {
                        if let Some(key) = fields.next() {
                            let mut value = fields.next().unwrap_or_default().to_string();
                            while let Some(f) = fields.next() {
                                value = value + " " + f;
                            }
                            if sender
                                .send(daemon::Operation::Incoming(key.to_string(), value))
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                return;
            }
        }
    }
}
