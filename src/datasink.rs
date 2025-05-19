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
