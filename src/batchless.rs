// Jobs::JobManager for systems without a job queue.
//
// In this system, the job ID of a process is found by walking the tree of jobs from the PID until
// we reach a process that is directly below a session leader, and then taking that process's PID as
// the job ID.  (The session leader is a process whose session PID is its own pid.)  If the process
// we're given is a session leader then we take its own PID to be the job ID.  Everything else ends
// up at the init process, which is probably OK.
//
// There's a remote possibility that the job ID will be reused during the lifetime of the system,
// confusing our statistics.  On Linux, the PIDs wrap around at about 4e6.  However, the job ID is
// not used in isolation (at the moment), but always with the user and the command name.  Should we
// want to add even more information we could incorporate eg the PID of the session leader, and/or
// the boot time of the system.

use crate::jobs;
use crate::process;

pub struct BatchlessJobManager {
    
}

impl BatchlessJobManager {

    fn maybe_init(&mut self, processes: &[process::Process]) {
        
    }

    fn lookup(&mut self, processes: &[process::Process], pid: &String) -> Option<process::Process> {
        None
    }
}

impl jobs::JobManager for BatchlessJobManager {
    fn job_id_from_pid(&mut self, pid: usize, processes: &[process::Process]) -> usize {
/*
        self.maybe_init(processes);
        if let Some(proc) = lookup(processes, &pid) {
            if proc.session == "0" {
                proc.session
            } else if proc.session == pid {
                pid
            } else {
                loop {
                    if let Some(parent) = lookup(processes, &proc.session) {
                        if parent.session == parent.pid {
                            break pid
                        } else {
                            pid = parent.pid;
                            proc = parent
                        }
                    } else {
                        break pid
                    }
                }
            }
        } else {
            "0".to_string()
        }.parse::<usize>().unwrap()
*/
        0
    }

    fn need_process_tree(&mut self) -> bool {
        true
    }
}

