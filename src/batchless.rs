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
    fn lookup<'a>(&self, processes: &'a [process::Process], pid: usize) -> Option<&'a process::Process> {
        // Sequential search may be too slow in practice
        let mut i = 0;
        while i < processes.len() {
            if processes[i].pid == pid {
                return Some(&processes[i])
            }
            i += 1
        }
        return None
    }
}

impl jobs::JobManager for BatchlessJobManager {
    fn job_id_from_pid(&mut self, pid: usize, processes: &[process::Process]) -> usize {
        let mut probe = self.lookup(processes, pid);
        if probe.is_none() {
            // Lost process is job 0
            0
        } else {
            loop {
                let proc = probe.unwrap();
                if proc.session == 0 {
                    // System process is its own job
                    break proc.session
                }
                if proc.session == pid {
                    // Session leader is its own job
                    break proc.session
                }
                let probe_parent = self.lookup(processes, proc.ppid);
                if probe_parent.is_none() {
                    // Orphaned subprocess is its own job
                    break proc.session
                }
                let parent = probe_parent.unwrap();
                if parent.pid == parent.session {
                    // Parent process is session leader, so this process is the job root
                    break proc.pid
                }
                probe = probe_parent
            }
        }
    }
    
    fn need_process_tree(&self) -> bool {
        true
    }
}

