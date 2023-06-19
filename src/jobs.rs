// A "job manager" that allows pids to be mapped to job numbers in a reliable way, this abstracts
// the job queue (if any) away from the rest of sonar.

use crate::process;

pub trait JobManager {
    fn job_id_from_pid(&mut self, pid: usize, processes: &[process::Process]) -> usize;
}
