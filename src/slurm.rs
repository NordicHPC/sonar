use crate::systemapi;
#[cfg(test)]
use crate::mocksystem;

#[cfg(test)]
use std::collections::HashMap;

// This returns Some(n) where n > 0 if we could parse the job ID, Some(0) if the overall pattern
// matched but the ID was not parseable, or None otherwise.  Thus None is a signal to fall back to
// other (non-Slurm) mechanisms.

pub fn get_job_id(system: &dyn systemapi::SystemAPI, pid: usize) -> Option<usize> {
    match system.get_procfs().read_to_string(&format!("{pid}/cgroup")) {
        Ok(text) => {
            // We want \1 of the first line that matches "/job_(.*?)/"
            //
            // The reason is that there can be several lines in that file that look roughly like
            // this, with different contents (except for the job info) but with the pattern the
            // same:
            //
            //    10:devices:/slurm/uid_2101171/job_280678/step_interactive/task_0
            //
            // It could be that we should match all of `/slurm.*/job_(\d+)/` to be on the safe side,
            // everything I've seen fits that, but not all use `/slurm/`, as seen in the test cases
            // below.

            for l in text.split('\n') {
                if let Some(x) = l.find("/job_") {
                    if let Some(y) = l[x + 5..].find('/') {
                        // Pattern found, so commit
                        return match l[x + 5..x + 5 + y].trim().parse::<usize>() {
                            Ok(n) => Some(n),
                            _ => Some(0),
                        };
                    }
                }
            }
            // Readable lines exhausted
            None
        }
        Err(_) => None,
    }
}

#[test]
fn test_get_job_id() {
    let mut files = HashMap::new();
    // This first test case from a Fox compute node on 2025-03-13
    files.insert("1337/cgroup".to_string(),
                 "0::/system.slice/slurmstepd.scope/job_1392969/step_0/user/task_0\n".to_string());
    // This second case from some older data
    files.insert("1336/cgroup".to_string(),
                 "10:devices:/slurm/uid_2101171/job_280678/step_interactive/task_0\n".to_string());
    // This third case the full output from Slurm data on eX3 (Slurm 21)
    files.insert("1335/cgroup".to_string(),
                 r#"13:freezer:/slurm/uid_7319/job_748468/step_0
12:cpu,cpuacct:/system.slice/slurmd.service
11:hugetlb:/
10:blkio:/system.slice/slurmd.service
9:devices:/system.slice/slurmd.service
8:misc:/
7:cpuset:/
6:rdma:/
5:net_cls,net_prio:/
4:pids:/system.slice/slurmd.service
3:perf_event:/
2:memory:/system.slice/slurmd.service
1:name=systemd:/system.slice/slurmd.service
"#.to_string());
    // This same as the previous one but I've scrambled the lines
    files.insert("1334/cgroup".to_string(),
                 r#"12:cpu,cpuacct:/system.slice/slurmd.service
11:hugetlb:/
10:blkio:/system.slice/slurmd.service
9:devices:/system.slice/slurmd.service
8:misc:/
7:cpuset:/
13:freezer:/slurm/uid_7319/job_748468/step_0
6:rdma:/
5:net_cls,net_prio:/
4:pids:/system.slice/slurmd.service
3:perf_event:/
2:memory:/system.slice/slurmd.service
1:name=systemd:/system.slice/slurmd.service
"#.to_string());
    // Some garbage cases
    files.insert("1338/cgroup".to_string(),
                 "random garbage\n".to_string());
    files.insert("1339/cgroup".to_string(),
                 "/job_hello/\n".to_string());
    let system = mocksystem::MockSystem::new().with_files(files).freeze();

    let r = get_job_id(&system, 1337);
    assert!(r.is_some());
    assert!(r.unwrap() == 1392969);

    let r = get_job_id(&system, 1336);
    assert!(r.is_some());
    assert!(r.unwrap() == 280678);

    let r = get_job_id(&system, 1335);
    assert!(r.is_some());
    assert!(r.unwrap() == 748468);

    let r = get_job_id(&system, 1334);
    assert!(r.is_some());
    assert!(r.unwrap() == 748468);

    let r = get_job_id(&system, 1338);
    assert!(r.is_none());

    let r = get_job_id(&system, 1339);
    assert!(r.is_some());
    assert!(r.unwrap() == 0);
}
