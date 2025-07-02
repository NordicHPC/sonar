pub mod directory;
#[cfg(feature = "kafka")]
pub mod kafka;
pub mod stdio;

use crate::systemapi::SystemAPI;

// The DataSink hides the specific data sink we use.  It receives outgoing traffic by `post()` and
// posts any incoming messages or errors on `sender`.  The sink may batch outgoing messages, and its
// network connection - if there is one - may go up and down, and so on.

pub trait DataSink {
    // Queue the message for sending, to be sent within the sending window (if applicable).
    fn post(
        &self,
        system: &dyn SystemAPI,
        topic_prefix: &Option<String>,
        cluster: &str,
        data_tag: &str,
        hostname: &str,
        value: String,
    );

    // Stop the sink. Nobody should be calling post() after calling stop().  Furthermore, the
    // DataSink object should be dropped as soon as possible after being stopped.
    fn stop(&self);
}
