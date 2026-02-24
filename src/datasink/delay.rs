// A data sink that holds the output for an initial duration, then passes everything through.

use crate::daemon::Dur;
use crate::datasink::DataSink;
use crate::posix::time::unix_now;
use crate::systemapi::SystemAPI;

struct Msg {
    topic_prefix: Option<String>,
    cluster: String,
    data_tag: String,
    hostname: String,
    value: String,
}

pub struct DelaySink {
    delaying: bool,
    deadline: u64,
    sink: Box<dyn DataSink>,
    held: Vec<Msg>,
}

impl DelaySink {
    pub fn new(delay: Dur, sink: Box<dyn DataSink>) -> DelaySink {
        DelaySink {
            delaying: true,
            deadline: unix_now() + delay.to_seconds(),
            sink,
            held: Vec::new(),
        }
    }
}

impl DataSink for DelaySink {
    fn post(
        &mut self,
        system: &dyn SystemAPI,
        topic_prefix: &Option<String>,
        cluster: &str,
        data_tag: &str,
        hostname: &str,
        value: String,
    ) {
        if self.delaying {
            self.held.push(Msg {
                topic_prefix: topic_prefix.clone(),
                cluster: cluster.to_string(),
                data_tag: data_tag.to_string(),
                hostname: hostname.to_string(),
                value,
            });
            if unix_now() >= self.deadline {
                self.delaying = false;
                for m in self.held.drain(0..) {
                    self.sink.post(
                        system,
                        &m.topic_prefix,
                        &m.cluster,
                        &m.data_tag,
                        &m.hostname,
                        m.value,
                    );
                }
            }
        } else {
            self.sink
                .post(system, topic_prefix, cluster, data_tag, hostname, value);
        }
    }

    fn stop(&mut self, system: &dyn SystemAPI) {
        for m in self.held.drain(0..) {
            self.sink.post(
                system,
                &m.topic_prefix,
                &m.cluster,
                &m.data_tag,
                &m.hostname,
                m.value,
            );
        }
    }
}
