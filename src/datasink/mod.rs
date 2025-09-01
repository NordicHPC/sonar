pub mod delay;
pub mod directory;
#[cfg(feature = "kafka")]
pub mod kafka;
pub mod stdio;

use crate::systemapi::SystemAPI;

// The DataSink hides the specific data sink we use.  It receives outgoing traffic by `post()` and
// posts any incoming messages or errors on `sender`.  The sink may batch outgoing messages, and its
// network connection - if there is one - may go up and down, and so on.

pub trait DataSink {
    // Queue the message for sending, to be sent within the sending window (when applicable).
    fn post(
        &mut self,
        system: &dyn SystemAPI,
        topic_prefix: &Option<String>,
        cluster: &str,
        data_tag: &str,
        hostname: &str,
        value: String,
    );

    // Stop the sink, attempt to send any queued messages, and wait for those sends to complete or
    // time out.  Nobody should be calling post() after calling stop().  Furthermore, the DataSink
    // object should be dropped as soon as possible after being stopped.  The flushing should be
    // best-effort, stop() should not block for a long time.  Sometimes the output can't be sent
    // because the receiver is not reachable; in that case, and others, output may be lost.
    fn stop(&mut self, system: &dyn SystemAPI);
}
