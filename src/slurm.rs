// Abstraction of jobs::JobManager for SLURM.

use crate::jobs;
use crate::process;

use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct SlurmJobManager {}

impl jobs::JobManager for SlurmJobManager {
    fn job_id_from_pid(&mut self, pid: usize, _processes: &[process::Process]) -> usize {
        let slurm_job_id = get_slurm_job_id(pid).unwrap_or_default();
        slurm_job_id.trim().parse::<usize>().unwrap_or_default()
    }
}

fn get_slurm_job_id(pid: usize) -> Option<String> {
    match File::open(format!("/proc/{pid}/cgroup")) {
        Ok(f) => {
            // We want \1 of the first line that matches "/job_(.*?)/"
            //
            // The reason is that there are several lines in that file that look roughly like this,
            // with different contents (except for the job info) but with the pattern the same:
            //
            //    10:devices:/slurm/uid_2101171/job_280678/step_interactive/task_0

            for l in BufReader::new(f).lines() {
                if let Ok(l) = l {
                    if let Some(x) = l.find("/job_") {
                        if let Some(y) = l[x + 5..].find('/') {
                            return Some(l[x + 5..x + 5 + y].to_string());
                        }
                    }
                } else {
                    return None;
                }
            }
            None
        }
        Err(_) => None,
    }
}
