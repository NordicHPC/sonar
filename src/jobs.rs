pub trait JobManager {
    fn job_id_from_pid(&mut self, pid: String) -> usize;
}
