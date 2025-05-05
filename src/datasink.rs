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

    // Stop the sink. Nobody should be calling post() after calling stop().  Furthermore, the
    // DataSink object should be dropped as soon as possible after being stopped.
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
        control_and_errors: mpsc::Sender<daemon::Operation>,
    ) -> StdioSink {
        thread::spawn(move || {
            control_message_reader(control_topic, control_and_errors);
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
        // TODO: This (maybe) needs to kill the stdin thread.  But it's probably doesn't need to
        // happen, and it's not clear how it could happen, there's no obvious signalling facility
        // for threads.
    }
}

fn control_message_reader(
    control_topic: String,
    control_and_errors: mpsc::Sender<daemon::Operation>,
) {
    for line in io::stdin().lines() {
        match line {
            Ok(s) => {
                let mut fields = s.split_ascii_whitespace();
                if let Some(topic) = fields.next() {
                    if control_topic == topic {
                        if let Some(key) = fields.next() {
                            let mut value = fields.next().unwrap_or_default().to_string();
                            for f in fields {
                                value = value + " " + f;
                            }
                            if control_and_errors
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
