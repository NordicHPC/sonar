// A "job manager" allows pids to be mapped to job numbers in a reliable way, this abstracts the job
// queue (if any) away from the rest of sonar.

use crate::systemapi;
use crate::types::{JobID, Pid};

use std::collections::HashMap;

pub trait JobManager {
    // Compute (job_id,is_slurm) from a process ID.
    //
    // There's an assumption here that the process map is always the same for all lookups
    // performed on a particular instance of JobManager.
    fn job_id_from_pid(
        &self,
        system: &dyn systemapi::SystemAPI,
        pid: Pid,
        processes: &HashMap<Pid, systemapi::Process>,
    ) -> (JobID, bool);
}

pub struct NoJobManager {}

impl NoJobManager {
    pub fn new() -> NoJobManager {
        NoJobManager {}
    }
}

impl JobManager for NoJobManager {
    fn job_id_from_pid(
        &self,
        _system: &dyn systemapi::SystemAPI,
        _pid: Pid,
        _processes: &HashMap<Pid, systemapi::Process>,
    ) -> (JobID, bool) {
        (0, false)
    }
}

pub struct AnyJobManager {
    force_slurm: bool,
}

impl AnyJobManager {
    pub fn new(force_slurm: bool) -> AnyJobManager {
        AnyJobManager { force_slurm }
    }
}

impl JobManager for AnyJobManager {
    fn job_id_from_pid(
        &self,
        system: &dyn systemapi::SystemAPI,
        pid: Pid,
        processes: &HashMap<Pid, systemapi::Process>,
    ) -> (JobID, bool) {
        if let Some(id) = system.compute_slurm_job_id(pid) {
            (id, id != 0)
        } else if let Some(p) = processes.get(&pid) {
            (p.pgrp as JobID, self.force_slurm)
        } else {
            (0, false)
        }
    }
}
