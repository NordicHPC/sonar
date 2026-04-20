// The HttpSink provides exfiltration over HTTP POST in a native way, not to the Kafka proxy.
//
// Messages are POSTed to "{api_root}/{cluster}/{node}/{topic}/{timestamp}".  Topic is either the
// <data-type> ("sample", etc) or <prefix>.<data-type> if a topic prefix has been configured.  The
// back-end must handle this, or disallow the use of prefixes.  The timestamp is a second count
// since epoch and is the time of generation of the message, though not necessarily exactly the same
// time as is *in* the message.
//
// Note HTTP messages are not batched.  Partly this is because the URL contains the timestamp so
// they can't be batched, and partly we'd have to set up the server so that it can handle multiple
// data for the same host and type in the same batch.  This is generally a headache.  It's better to
// instead look forward to when the connection may be kept open.  In reality, for most nodes,
// traffic will be low and non-batching is not an issue.
//
// TODO: Should we require the timestamp in the envelope to match the message?

use crate::daemon::{HttpIni, Ini, Operation};
use crate::datasink::background::{background_producer, BackgroundSender, Message, Size};
use crate::datasink::http_upload;
use crate::datasink::DataSink;
use crate::systemapi::SystemAPI;
use crossbeam::channel;
use std::thread;

pub struct HttpMsg {
    pub cluster: String,
    pub node: String,
    pub topic: String,
    pub timestamp: u64,
    pub value: String,
}

impl Size for HttpMsg {
    fn size(&self) -> usize {
        self.value.len()
    }
}

pub struct HttpSink {
    outgoing_message_queue: channel::Sender<Message<HttpMsg>>,
    producer: Option<thread::JoinHandle<()>>,
}

impl HttpSink {
    pub fn new(ini: &Ini, control_and_errors: channel::Sender<Operation>) -> HttpSink {
        let (outgoing_message_queue, incoming_message_queue) = channel::unbounded();
        let settings = ini.http.clone();
        let curl = if let Some(curl) = &ini.programs.curl_cmd {
            curl.clone()
        } else {
            "curl".to_string()
        };
        let producer = thread::spawn(move || {
            http_producer(curl, settings, incoming_message_queue, control_and_errors)
        });
        HttpSink {
            outgoing_message_queue,
            producer: Some(producer),
        }
    }
}

impl DataSink for HttpSink {
    fn post(
        &mut self,
        system: &dyn SystemAPI,
        topic_prefix: &Option<String>,
        cluster: &str,
        data_type: &str,
        hostname: &str,
        value: String,
    ) {
        let topic = if let Some(prefix) = topic_prefix {
            prefix.clone() + "." + data_type
        } else {
            data_type.to_string()
        };
        let _ = self.outgoing_message_queue.send(Message::M(HttpMsg {
            cluster: cluster.to_string(),
            node: hostname.to_string(),
            topic,
            timestamp: system.get_now_in_secs_since_epoch(),
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

fn http_producer(
    curl_cmd: String,
    settings: HttpIni,
    incoming_message_queue: channel::Receiver<Message<HttpMsg>>,
    control_and_errors: channel::Sender<Operation>,
) {
    let uploader = http_upload::HttpUploader::new(
        &curl_cmd,
        &settings.http_proxy,
        settings.timeout.to_seconds(),
    );
    let op = HttpBackgroundProducer {
        uploader,
        settings: settings.clone(),
        control_and_errors,
    };
    background_producer(incoming_message_queue, &op);
}

pub struct HttpBackgroundProducer<'a> {
    uploader: http_upload::HttpUploader<'a>,
    settings: HttpIni,
    control_and_errors: channel::Sender<Operation>,
}

impl<'a> BackgroundSender<HttpMsg> for HttpBackgroundProducer<'a> {
    fn send_all(&self, _id: usize, backlog: Vec<HttpMsg>) {
        let api_root = &self.settings.api_root;
        for HttpMsg {
            cluster,
            node,
            topic,
            timestamp,
            value,
        } in backlog
        {
            let cred = if let Some(passwd) = &self.settings.upload_password {
                Some(http_upload::Credential::from_user_passwd(
                    cluster.as_str(),
                    passwd.as_str(),
                    self.settings.upload_password_file.is_none(),
                ))
            } else {
                None
            };
            let url = format!("{api_root}/{cluster}/{node}/{topic}/{timestamp}");
            match self.uploader.start(&url, &cred) {
                Ok(stream) => {
                    stream.put_string(value);
                    if let Err(e) = stream.end() {
                        let _ = self
                            .control_and_errors
                            .send(Operation::MessageDeliveryError(e));
                    }
                }
                Err(e) => {
                    // Not found, permission denied, etc.
                    let _ = self.control_and_errors.send(Operation::Fatal(format!(
                        "Failed to start uploader: {:?}",
                        e
                    )));
                }
            }
        }
    }

    fn sending_window_s(&self) -> u64 {
        self.settings.sending_window.to_seconds()
    }

    fn shutdown_delay_ms(&self) -> u64 {
        5000
    }

    fn batch_size(&self) -> Option<usize> {
        // No batching for HTTP POST upload, see top comment.
        None
    }

    fn metadata_size(&self) -> (usize, usize) {
        (0, 0)
    }
}
