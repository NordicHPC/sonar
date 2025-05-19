use crate::daemon;
use crate::datasink::DataSink;

use std::io;
use std::sync::mpsc;
use std::thread;

// Data sink that dumps the output as JSON into a date-keyed directory tree.  It reads no command messages.

pub struct DirectorySink {
    data_dir: String,
}

impl DirectorySink {
    pub fn new(
        data_dir: &str,
    ) -> DirectorySink {
        DirectorySink { data_dir: data_dir.to_string() }
    }
}

impl DataSink for DirectorySink {
    // The key is the host name.
    // The topic must be parsed to find out where to dump the data.
    fn post(&self, topic: String, key: String, value: String) {
        // TODO
    }

    fn stop(&self) {
        // Nothing to do
    }
}
