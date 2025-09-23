#[cfg(test)]
use crate::linux::mocksystem;

#[cfg(test)]
use std::collections::HashMap;

use crate::linux::slurm::get_job_id;

#[test]
fn test_get_job_id() {
    let mut files = HashMap::new();
    // This first test case from a Fox compute node on 2025-03-13
    files.insert(
        "1337/cgroup".to_string(),
        "0::/system.slice/slurmstepd.scope/job_1392969/step_0/user/task_0\n".to_string(),
    );
    // This second case from some older data
    files.insert(
        "1336/cgroup".to_string(),
        "10:devices:/slurm/uid_2101171/job_280678/step_interactive/task_0\n".to_string(),
    );
    // This third case the full output from Slurm data on eX3 (Slurm 21)
    files.insert(
        "1335/cgroup".to_string(),
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
"#
        .to_string(),
    );
    // This same as the previous one but I've scrambled the lines
    files.insert(
        "1334/cgroup".to_string(),
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
"#
        .to_string(),
    );
    // Some garbage cases
    files.insert("1338/cgroup".to_string(), "random garbage\n".to_string());
    files.insert("1339/cgroup".to_string(), "/job_hello/\n".to_string());
    let system = mocksystem::Builder::new().with_proc_files(files).freeze();
    let fs = system.get_procfs();

    let r = get_job_id(fs, 1337);
    assert!(r.is_some());
    assert!(r.unwrap() == 1392969);

    let r = get_job_id(fs, 1336);
    assert!(r.is_some());
    assert!(r.unwrap() == 280678);

    let r = get_job_id(fs, 1335);
    assert!(r.is_some());
    assert!(r.unwrap() == 748468);

    let r = get_job_id(fs, 1334);
    assert!(r.is_some());
    assert!(r.unwrap() == 748468);

    let r = get_job_id(fs, 1338);
    assert!(r.is_none());

    let r = get_job_id(fs, 1339);
    assert!(r.is_some());
    assert!(r.unwrap() == 0);
}
