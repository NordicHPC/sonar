use crate::daemon::Operation;
use crate::datasink::DataSink;
use crate::systemapi::SystemAPI;
use crate::time;

use std::io::Write;

use crossbeam::channel;

// Data sink that dumps the output as JSON into a date-keyed directory tree.  It reads no command
// messages.

pub struct DirectorySink {
    data_dir: String,
    control_and_errors: channel::Sender<Operation>,
}

impl DirectorySink {
    pub fn new(data_dir: &str, control_and_errors: channel::Sender<Operation>) -> DirectorySink {
        DirectorySink {
            data_dir: data_dir.to_string(),
            control_and_errors,
        }
    }
}

impl DataSink for DirectorySink {
    fn post(
        &mut self,
        system: &dyn SystemAPI,
        _topic_prefix: &Option<String>,
        _cluster: &str,
        mut data_tag: &str,
        hostname: &str,
        value: String,
    ) {
        if data_tag == "cluster" {
            data_tag = "cluzter";
        }
        let data_attribute = match data_tag {
            "sysinfo" | "sample" => hostname,
            _ => "slurm",
        };
        let basename = format!("0+{data_tag}-{data_attribute}.json");
        let timestamp = system.get_now_in_secs_since_epoch();
        let (yyyy, mz, dz, _, _, _) = time::unix_time_components(timestamp);
        let dirname = format!("{}/{:02}/{:02}", yyyy, mz + 1, dz + 1);
        let directory = self.data_dir.clone() + "/" + &dirname;
        let filename = directory.clone() + "/" + &basename;

        let mut db = std::fs::DirBuilder::new();
        db.recursive(true);
        if db.create(&directory).is_err() {
            let _ = self
                .control_and_errors
                .send(Operation::MessageDeliveryError(
                    "Can't create directory".to_string(),
                ));
            return;
        }
        let mut file = match std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&filename)
        {
            Err(_) => {
                let _ = self
                    .control_and_errors
                    .send(Operation::MessageDeliveryError(
                        "Can't open file for append".to_string(),
                    ));
                return;
            }
            Ok(f) => f,
        };
        match file.write_all(value.as_bytes()) {
            Err(_) => {
                let _ = self
                    .control_and_errors
                    .send(Operation::MessageDeliveryError(
                        "Can't write to file".to_string(),
                    ));
            }
            Ok(_) => {}
        }
    }

    fn stop(&mut self, _system: &dyn SystemAPI) {
        // Nothing to do
    }
}
