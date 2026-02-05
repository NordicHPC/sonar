use crate::jobsapi;
use crate::systemapi;
use crate::types::JobID;

use std::collections::HashMap;

pub struct MockJobManager {}

impl jobsapi::JobManager for MockJobManager {
    fn job_id_from_pid(
        &self,
        _system: &dyn systemapi::SystemAPI,
        pid: Pid,
        _processes: &HashMap<Pid, Box<systemapi::Process>>,
    ) -> (JobID, bool) {
        (pid as JobID, false)
    }
}
