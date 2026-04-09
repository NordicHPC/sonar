use crate::util::rng::Rng;
use crossbeam::channel;
use std::thread;
use std::time::{Duration, Instant};

pub struct Msg {
    pub timestamp: u64,
    pub topic: String,
    pub key: String,
    pub value: String,
}

impl Msg {
    // This can return an estimate, but it's better to overestimate than underestimate.
    pub fn size(&self) -> usize {
        return self.topic.len() + self.key.len() + self.value.len() + 20;
    }
}

pub enum Message {
    Stop,
    M(Msg),
}

pub trait BackgroundSender {
    fn send_all(&self, id: usize, msgs: Vec<Msg>);
    fn shutdown_delay_ms(&self) -> usize;
}

pub fn background_producer(
    sending_window: u64,
    incoming_message_queue: channel::Receiver<Message>,
    sender: &dyn BackgroundSender,
) {
    let mut id = 0usize;
    let mut rng = Rng::new();
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
                let num = backlog.len();
                sender.send_all(id, backlog);
                backlog = vec![];
                id += num;
            }
            recv(incoming_message_queue) -> msg => match msg {
                Ok(Message::M(msg)) => {
                    backlog.push(msg);
                    must_arm = !armed;
                }
                Ok(Message::Stop) | Err(_) => {
                    if backlog.len() > 0 {
                        _ = sender.send_all(id, backlog);
                    }
                    break 'producer_loop;
                }
            }
        }
    }

    // Best effort: give the Kafka thread an opportunity to send what it has.
    thread::sleep(Duration::from_millis(2 * sender.shutdown_delay_ms() as u64));
}
