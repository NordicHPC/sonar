// A "job manager" allows pids to be mapped to job numbers in a reliable way, this abstracts the job
// queue (if any) away from the rest of sonar.

use crate::process;

pub trait JobManager {
    // Compute a job ID from a process ID.
    //
    // There's an assumption here that the process slice is always the same for all lookups
    // performed on a particular instance of JobManager.
    fn job_id_from_pid(&mut self, pid: usize, processes: &[process::Process]) -> usize;

    // Return true iff the full process tree is required for the job ID computation, otherwise a
    // filtered process tree can be used.
    fn need_process_tree(&self) -> bool;
}
