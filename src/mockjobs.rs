use crate::jobsapi;
use crate::procfs;
use crate::systemapi;

use std::collections::HashMap;

pub struct MockJobManager {}

impl jobsapi::JobManager for MockJobManager {
    fn job_id_from_pid(
        &self,
        _system: &dyn systemapi::SystemAPI,
        pid: usize,
        _processes: &HashMap<usize, procfs::Process>,
    ) -> (usize, bool) {
        (pid, false)
    }
}
