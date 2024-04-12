// A "job manager" allows pids to be mapped to job numbers in a reliable way, this abstracts the job
// queue (if any) away from the rest of sonar.

use crate::procfs;

pub trait JobManager {
    // After process extraction, preprocess process data for the job manager type.  This may alter
    // some process data.
    fn preprocess(&mut self, processes: Vec<procfs::Process>) -> Vec<procfs::Process>;

    // Compute a job ID from a process ID.
    //
    // There's an assumption here that the process slice is always the same for all lookups
    // performed on a particular instance of JobManager, and that this slice is of the full vector
    // returned from preprocess(), above.
    fn job_id_from_pid(&mut self, pid: usize, processes: &[procfs::Process]) -> usize;
}
