// Generic logic for sending messages in the background with time delays and batching.

use crate::util::rng::Rng;
use crossbeam::channel;
use std::cmp::max;
use std::thread;
use std::time::{Duration, Instant};

pub trait Size {
    // Estimate the size of a message.  This can return an estimate, but it's better to overestimate
    // than underestimate, as the value will be used in creating batches that don't blow by network,
    // proxy, or server limits.
    fn size(&self) -> usize;
}

pub enum Message<Msg: Size> {
    Stop,
    M(Msg),
}

pub trait BackgroundSender<Msg: Size> {
    // Send all the messages in msgs, together if possible (batching is done at a higher level).
    fn send_all(&self, id: usize, msgs: Vec<Msg>);

    // If random delay before sending, this is the ceiling on how log to delay, may be zero.
    fn sending_window_s(&self) -> u64;

    // How long to wait for a backgrounded sender to send things when shutting down Sonar, may be
    // zero.
    fn shutdown_delay_ms(&self) -> u64;

    // Ceiling on the batch size, if messages can be batched.
    fn batch_size(&self) -> Option<usize>;

    // Estimated size of metadata in bytes, when batching: per-batch and per-message.  If batching
    // is disabled (cutoff is zero) then this will not be called, but otherwise it should
    // conservatively estimate the size of the message metadata.
    fn metadata_size(&self) -> (usize, usize);
}

// Call background_producer from a dedicated producer thread (or spawn it as a thread).  It will
// loop on the incoming_message_queue until it receives a stop message.  The messages received are
// held for a suitable random interval in the sending_window and then sent.
pub fn background_producer<Msg: Size>(
    incoming_message_queue: channel::Receiver<Message<Msg>>,
    sender: &dyn BackgroundSender<Msg>,
) {
    // Simplifying assumption for now, if no window then make a window of 1s.
    let sending_window = max(sender.sending_window_s(), 1);
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
                send_all(sender, id, backlog);
                backlog = vec![];
                id += num;
            }
            recv(incoming_message_queue) -> msg => match msg {
                Ok(Message::M(msg)) => {
                    backlog.push(msg);
                    must_arm = !armed;
                }
                Ok(Message::Stop) | Err(_) => {
                    send_all(sender, id, backlog);
                    break 'producer_loop;
                }
            }
        }
    }

    // Best effort: give the sending thread an opportunity to send what it has.
    thread::sleep(Duration::from_millis(sender.shutdown_delay_ms() as u64));
}

// Send all messages in the backlog, but apply batching if appropriate.
// Note backlog length may be zero, do nothing if so.
fn send_all<Msg: Size>(sender: &dyn BackgroundSender<Msg>, mut id: usize, mut backlog: Vec<Msg>) {
    if backlog.is_empty() {
        return;
    }

    // Note, the /Sending {} items/ pattern is used by regression tests.
    log::debug!("Sending {} items", backlog.len());

    let cutoff = sender.batch_size().unwrap_or(0);
    if cutoff == 0 {
        sender.send_all(id, backlog);
        return;
    }

    let (batch_overhead, msg_overhead) = sender.metadata_size();
    while !backlog.is_empty() {
        let mut i = 0;
        let mut sz = batch_overhead;
        while i < backlog.len() {
            let newsz = sz + backlog[i].size() + msg_overhead;
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
            // Try to send it anyway, take the consequences elsewhere.
            i = 1;
        }
        let new_backlog = backlog.split_off(i);
        let to_send = backlog;
        let num_sent = to_send.len();
        backlog = new_backlog;
        sender.send_all(id, to_send);
        id += num_sent;
    }
}
