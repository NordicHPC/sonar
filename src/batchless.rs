// jobs::JobManager for systems without a job queue.
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
//
// There's also a challenge with this scheme in that, since the output is keyed on program name as
// well as on user name and job ID, multiple output lines are going to have the same user name and
// job ID in a tree of *different* processes (ie where subprocesses of the root process exec
// something else).  This is not wrong but it is something that the consumer must take into account.
// For example, in assessing the resources for a job, the resources for all the different programs
// for the job must be taken into account.
//
// (Also this puts some more pressure on the reused-PID problem.)

use crate::jobs;
#[cfg(test)]
use crate::jobs::JobManager;
use crate::process;
use std::collections::HashMap;

pub struct BatchlessJobManager {
    // Process tables can be large and searching them sequentially for every lookup will be slow, so
    // add a cache.  Various structures could work.  Here a hashmap maps pid -> (session, ppid).
    cache: HashMap<usize, (usize, usize)>,
}

impl BatchlessJobManager {
    pub fn new() -> BatchlessJobManager {
        BatchlessJobManager {
            cache: HashMap::new(),
        }
    }
}

impl BatchlessJobManager {
    fn lookup(&mut self, processes: &[process::Process], pid: usize) -> Option<(usize, usize)> {
        let probe = self.cache.get(&pid);
        if probe.is_some() {
            return probe.copied();
        }

        let mut i = 0;
        while i < processes.len() {
            if processes[i].pid == pid {
                let entry = (processes[i].session, processes[i].ppid);
                self.cache.insert(pid, entry);
                return Some(entry);
            }
            i += 1
        }

        None
    }
}

impl jobs::JobManager for BatchlessJobManager {
    fn job_id_from_pid(&mut self, mut proc_pid: usize, processes: &[process::Process]) -> usize {
        let mut probe = self.lookup(processes, proc_pid);
        if probe.is_none() {
            // Lost process is job 0
            0
        } else {
            loop {
                let (proc_session, parent_pid) = probe.unwrap();
                if proc_session == 0 {
                    // System process is its own job
                    break proc_session;
                }
                if proc_session == proc_pid {
                    // Session leader is its own job
                    break proc_session;
                }
                let probe_parent = self.lookup(processes, parent_pid);
                if probe_parent.is_none() {
                    // Orphaned subprocess is its own job
                    break proc_session;
                }
                let (parent_session, _) = probe_parent.unwrap();
                if parent_pid == parent_session {
                    // Parent process is session leader, so this process is the job root
                    break proc_pid;
                }
                proc_pid = parent_pid;
                probe = probe_parent;
            }
        }
    }

    fn need_process_tree(&self) -> bool {
        true
    }
}

#[test]
fn test_batchless_jobs() {
    let mut jm = BatchlessJobManager::new();
    let procs = process::parsed_full_test_output();
    assert!(jm.job_id_from_pid(82554, &procs) == 82329); // firefox subprocess -> firefox, b/c firefox is below session leader
    assert!(jm.job_id_from_pid(82329, &procs) == 82329); // firefox -> firefox
    assert!(jm.job_id_from_pid(1, &procs) == 1); // init
    assert!(jm.job_id_from_pid(1805, &procs) == 1805); // sd-pam -> sd-pam, b/c 1804 is session leader
    assert!(jm.job_id_from_pid(232, &procs) == 0); // session 0
    assert!(jm.job_id_from_pid(74536, &procs) == 74536); // shell
    assert!(jm.job_id_from_pid(2305, &procs) == 2225); // ibus-extension- -> ibus-daemon, b/c ibus-daemon is below session leader
    assert!(jm.job_id_from_pid(200, &procs) == 0); // lost process
    assert!(jm.job_id_from_pid(80199, &procs) == 1823); // lost parent process
}
