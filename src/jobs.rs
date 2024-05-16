// A "job manager" allows pids to be mapped to job numbers in a reliable way, this abstracts the job
// queue (if any) away from the rest of sonar.

use crate::procfs;
use std::collections::HashMap;

pub trait JobManager {
    // Compute a job ID from a process ID.
    //
    // There's an assumption here that the process map is always the same for all lookups
    // performed on a particular instance of JobManager.
    fn job_id_from_pid(&mut self, pid: usize, processes: &HashMap<usize, procfs::Process>)
        -> usize;
}
