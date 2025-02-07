use crate::jobsapi;
use crate::procfs;

use std::collections::HashMap;

pub struct MockJobManager {}

impl jobsapi::JobManager for MockJobManager {
    fn job_id_from_pid(&self, pid: usize, _processes: &HashMap<usize, procfs::Process>) -> usize {
        pid
    }
}
