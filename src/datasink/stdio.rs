use crate::daemon;
use crate::datasink::DataSink;
use crate::systemapi::SystemAPI;

use std::io;
use std::sync::mpsc;
use std::thread;

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
    fn post(
        &self,
        _system: &dyn SystemAPI,
        topic_prefix: &Option<String>,
        cluster: &str,
        data_tag: &str,
        hostname: &str,
        value: String,
    ) {
        let prefix = if let Some(ref s) = topic_prefix {
            s.to_string() + "."
        } else {
            "".to_string()
        };
        println!(
            "{{\"topic\":\"{prefix}{cluster}.{data_tag}\",\n \"key\":\"{hostname}\",\n \"client\":\"{}\",\n \"value\":{value}}}",
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
