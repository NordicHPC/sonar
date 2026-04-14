use crate::util::rng::Rng;
use crossbeam::channel;
use std::thread;
use std::time::{Duration, Instant};

pub struct Msg {
    #[allow(unused)]
    pub timestamp: u64,
    pub topic: String,
    pub key: String,
    pub value: String,
}

impl Msg {
    // This can return an estimate, but it's better to overestimate than underestimate.
    pub fn size(&self) -> usize {
        self.topic.len() + self.key.len() + self.value.len() + 20
    }
}

pub enum Message {
    Stop,
    M(Msg),
}

pub trait BackgroundSender {
    // Send all the messages in msgs, together if possible (batching is done at a higher level).
    fn send_all(&self, id: usize, msgs: Vec<Msg>);

    // How long to wait for a backgrounded sender to send things when shutting down Sonar, may be
    // zero.
    fn shutdown_delay_ms(&self) -> usize;

    // Estimated size of metadata in bytes, when batching: per-batch and per-message.  If batching
    // is disabled (cutoff is zero) then this will not be called, but otherwise it should
    // conservatively estimate the size of the message metadata.
    fn metadata_size(&self) -> (usize, usize);
}

// Call background_producer from a dedicated producer thread (or spawn it as a thread).  It will
// loop on the incoming message queue until it receives a stop message.  The messages received are
// held for a suitable random interval and then sent.
pub fn background_producer(
    cutoff: Option<usize>,
    sending_window: u64,
    incoming_message_queue: channel::Receiver<Message>,
    sender: &dyn BackgroundSender,
) {
    let sending_window = if sending_window == 0 {
        1
    } else {
        sending_window
    };
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
                log::debug!("Sending window open.");
                let num = backlog.len();
                send_all(sender, cutoff, id, backlog);
                backlog = vec![];
                id += num;
            }
            recv(incoming_message_queue) -> msg => match msg {
                Ok(Message::M(msg)) => {
                    backlog.push(msg);
                    must_arm = !armed;
                }
                Ok(Message::Stop) | Err(_) => {
                    send_all(sender, cutoff, id, backlog);
                    break 'producer_loop;
                }
            }
        }
    }

    // Best effort: give the sending thread an opportunity to send what it has.
    thread::sleep(Duration::from_millis(2 * sender.shutdown_delay_ms() as u64));
}

// Send all messages in the backlog, but apply batching if appropriate.
// Note backlog length may be zero, do nothing if so.
fn send_all(
    sender: &dyn BackgroundSender,
    cutoff: Option<usize>,
    mut id: usize,
    mut backlog: Vec<Msg>,
) {
    if !backlog.is_empty() {
        // Note, the /Sending {} items/ pattern is used by regression tests.
        log::debug!("Sending {} items", backlog.len());
        let (boverhead, moverhead) = sender.metadata_size();
        if let Some(cutoff) = cutoff {
            while !backlog.is_empty() {
                let mut i = 0;
                let mut sz = boverhead;
                while i < backlog.len() {
                    let newsz = sz + backlog[i].size() + moverhead;
                    if newsz >= cutoff {
                        break;
                    }
                    sz = newsz;
                    i += 1;
                }
                if i == 0 {
                    log::error!(
                        "Message of size {} is too large to send, should not happen",
                        backlog[0].size()
                    );
                    // TODO: Must drop it or send it!!
                }
                let new_backlog = backlog.split_off(i);
                let to_send = backlog;
                let num_sent = to_send.len();
                backlog = new_backlog;
                sender.send_all(id, to_send);
                id += num_sent;
            }
        } else {
            sender.send_all(id, backlog);
        }
    }
}
