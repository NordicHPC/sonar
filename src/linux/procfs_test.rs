#![allow(clippy::len_zero)]

use crate::linux::mocksystem;
use crate::systemapi::SystemAPI;

use crate::linux::procfs;
use crate::systemapi;

use std::collections::HashMap;

// For the parse test we use the full text of stat and meminfo, but for stat we only want the
// 'btime' line and for meminfo we only want the 'MemTotal:' line.  Other tests can economize on the
// input.

#[test]
pub fn procfs_parse_test() {
    let pids = vec![(4018, 1000)];

    let mut users = HashMap::new();
    users.insert(1000, "zappa".to_string());

    let mut files = HashMap::new();
    files.insert(
        "stat".to_string(),
        std::include_str!("testdata/stat.txt").to_string(),
    );
    files.insert(
        "meminfo".to_string(),
        std::include_str!("testdata/meminfo.txt").to_string(),
    );
    files.insert(
        "4018/stat".to_string(),
        "4018 (firefox) S 2190 2189 2189 0 -1 4194560 19293188 3117638 1823 557 51361 15728 5390 2925 20 0 187 0 16400 5144358912 184775 18446744073709551615 94466859782144 94466860597976 140720852341888 0 0 0 0 4096 17663 0 0 0 17 4 0 0 0 0 0 94466860605280 94466860610840 94466863497216 140720852350777 140720852350820 140720852350820 140720852357069 0".to_string());
    files.insert(
        "4018/statm".to_string(),
        "1255967 185959 54972 200 0 316078 0".to_string(),
    );
    files.insert("4018/status".to_string(), "RssAnon: 12345 kB".to_string());
    files.insert(
        "loadavg".to_string(),
        "1.75 2.125 10.5 128/10340 12345".to_string(),
    );

    let ticks_per_sec = 100.0; // We define this
    let utime_ticks = 51361.0; // field(/proc/4018/stat, 14)
    let stime_ticks = 15728.0; // field(/proc/4018/stat, 15)
    let boot_time = 1698303295.0; // field(/proc/stat, "btime")
    let start_ticks = 16400.0; // field(/proc/4018/stat, 22)
    let rss: f64 = 185959.0 * 4.0; // pages_to_kib(field(/proc/4018/statm, 1))
    let memtotal = 16093776.0; // field(/proc/meminfo, "MemTotal:")
    let size = 316078 * 4; // pages_to_kib(field(/proc/4018/statm, 5))
    let rssanon = 12345; // field(/proc/4018/status, "RssAnon:")
    let load1 = 1.75;
    let load5 = 2.125;
    let load15 = 10.5;
    let runnable = 128;
    let existing = 10340;

    // now = boot_time + start_time + utime_ticks + stime_ticks + arbitrary idle time
    let now = (boot_time
        + (start_ticks / ticks_per_sec)
        + (utime_ticks / ticks_per_sec)
        + (stime_ticks / ticks_per_sec)
        + 2000.0) as u64;

    let system = mocksystem::Builder::new()
        .with_files(files)
        .with_pids(pids)
        .with_users(users)
        .with_now(now)
        .freeze();
    let fs = system.get_procfs();
    let memory = procfs::get_memory(fs).expect("Test: Must have data");
    assert!(memory.total == 16093776);
    assert!(memory.available == 8162068);
    let (total_secs, per_cpu_secs) = system.get_node_information().expect("Test: Must have data");
    let (mut info, _) = system
        .get_process_information()
        .expect("Test: Must have data");
    assert!(info.len() == 1);
    let mut xs = info.drain();
    let p = xs.next().expect("Test: Should have data").1;
    assert!(p.pid == 4018); // from enumeration of /proc
    assert!(p.uid == 1000); // ditto
    assert!(p.user == "zappa"); // from getent
    assert!(p.command == "firefox"); // field(/proc/4018/stat, 2)
    assert!(p.ppid == 2190); // field(/proc/4018/stat, 4)
    assert!(p.pgrp == 2189); // field(/proc/4018/stat, 5)

    let now_time = now as f64;
    let now_ticks = now_time * ticks_per_sec;
    let boot_ticks = boot_time * ticks_per_sec;
    let realtime_ticks = now_ticks - (boot_ticks + start_ticks);
    let cpu_pct_value = (utime_ticks + stime_ticks) / realtime_ticks;
    let cpu_pct = (cpu_pct_value * 1000.0).round() / 10.0;
    assert!(p.cpu_pct == cpu_pct);

    let mem_pct = (rss * 1000.0 / memtotal).round() / 10.0;
    assert!(p.mem_pct == mem_pct);

    assert!(p.mem_size_kib == size);
    assert!(p.rssanon_kib == rssanon);

    assert!(total_secs == (241155 + 582 + 127006 + 0 + 3816) / 100); // "cpu " line of "stat" data
    assert!(per_cpu_secs.len() == 8);
    assert!(per_cpu_secs[0] == (32528 + 189 + 19573 + 0 + 1149) / 100); // "cpu0 " line of "stat" data
    assert!(per_cpu_secs[7] == (27582 + 61 + 12558 + 0 + 426) / 100); // "cpu7 " line of "stat" data

    let (l1, l5, l15, r, e) = procfs::get_loadavg(fs).unwrap();
    assert!(load1 == l1);
    assert!(load5 == l5);
    assert!(load15 == l15);
    assert!(runnable == r);
    assert!(existing == e);
}

#[test]
pub fn procfs_parse_errors() {
    let mut files = HashMap::new();
    files.insert(
        "loadavg".to_string(),
        "1.75 2.125 10.5 128/ 10340 12345".to_string(),
    );
    let system = mocksystem::Builder::new().with_files(files).freeze();
    let fs = system.get_procfs();
    assert!(procfs::get_loadavg(fs).is_err());
}

#[test]
pub fn procfs_dead_and_undead_test() {
    let pids = vec![(4018, 1000), (4019, 1000), (4020, 1000)];

    let mut users = HashMap::new();
    users.insert(1000, "zappa".to_string());

    let mut files = HashMap::new();
    files.insert("stat".to_string(), "btime 1698303295".to_string());
    files.insert(
        "meminfo".to_string(),
        "MemTotal:       16093776 kB".to_string(),
    );
    files.insert(
        "4018/stat".to_string(),
        "4018 (firefox) S 2190 2189 2189 0 -1 4194560 19293188 3117638 1823 557 51361 15728 5390 2925 20 0 187 0 16400 5144358912 184775 18446744073709551615 94466859782144 94466860597976 140720852341888 0 0 0 0 4096 17663 0 0 0 17 4 0 0 0 0 0 94466860605280 94466860610840 94466863497216 140720852350777 140720852350820 140720852350820 140720852357069 0".to_string());
    files.insert(
        "4019/stat".to_string(),
        "4019 (firefox) Z 2190 2189 2189 0 -1 4194560 19293188 3117638 1823 557 51361 15728 5390 2925 20 0 187 0 16400 5144358912 184775 18446744073709551615 94466859782144 94466860597976 140720852341888 0 0 0 0 4096 17663 0 0 0 17 4 0 0 0 0 0 94466860605280 94466860610840 94466863497216 140720852350777 140720852350820 140720852350820 140720852357069 0".to_string());
    files.insert(
        "4020/stat".to_string(),
        "4020 (python3) X 0 -1 -1 0 -1 4243524 0 0 0 0 0 0 0 0 20 0 0 0 10643829 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0 0 0 0 0 0 0 0 0".to_string());

    files.insert(
        "4018/statm".to_string(),
        "1255967 185959 54972 200 0 316078 0".to_string(),
    );
    files.insert(
        "4019/statm".to_string(),
        "1255967 185959 54972 200 0 316078 0".to_string(),
    );
    files.insert(
        "4020/statm".to_string(),
        "1255967 185959 54972 200 0 316078 0".to_string(),
    );
    files.insert("4018/status".to_string(), "RssAnon: 12345 kB".to_string());
    files.insert("4019/status".to_string(), "RssAnon: 12345 kB".to_string());
    files.insert("4020/status".to_string(), "RssAnon: 12345 kB".to_string());

    let mut threads = HashMap::new();
    threads.insert(4018, vec![(4018, 1), (40181, 1), (40182, 1), (40183, 1)]);

    let system = mocksystem::Builder::new()
        .with_files(files)
        .with_pids(pids)
        .with_users(users)
        .with_threads(threads)
        .freeze();
    let (mut info, _) = system
        .get_process_information()
        .expect("Test: Must have data");

    // 4020 should be dropped - it's dead
    assert!(info.len() == 2);

    let mut xs = info.drain();
    let mut p = xs.next().expect("Test: Should have some data").1;
    let mut q = xs.next().expect("Test: Should have more data").1;
    if p.pid > q.pid {
        (p, q) = (q, p);
    }
    assert!(p.pid == 4018);
    assert!(p.command == "firefox");
    assert!(p.num_threads == 4);
    assert!(q.pid == 4019);
    assert!(q.command == "firefox <defunct>");
    assert!(q.num_threads == 1);
}

#[test]
pub fn procfs_cpuinfo_test_x86_64() {
    let mut files = HashMap::new();
    files.insert(
        "cpuinfo".to_string(),
        std::include_str!("testdata/cpuinfo-x86_64.txt").to_string(),
    );
    let system = mocksystem::Builder::new().with_files(files).freeze();
    let systemapi::CpuInfo {
        sockets,
        cores_per_socket,
        threads_per_core,
        cores,
    } = procfs::get_cpu_info_x86_64(system.get_procfs()).expect("Test: Must have data");
    assert!(cores[0].model_name.find("E5-2637").is_some());
    assert!(sockets == 2);
    assert!(cores_per_socket == 4);
    assert!(threads_per_core == 2);
}

#[test]
pub fn procfs_cpuinfo_test_aarch64() {
    let mut files = HashMap::new();
    files.insert(
        "cpuinfo".to_string(),
        std::include_str!("testdata/cpuinfo-aarch64.txt").to_string(),
    );
    let system = mocksystem::Builder::new().with_files(files).freeze();
    let systemapi::CpuInfo {
        sockets,
        cores_per_socket,
        threads_per_core,
        cores,
    } = procfs::get_cpu_info_aarch64(system.get_procfs()).expect("Test: Must have data");
    assert!(cores[0].model_name.find("ARMv8.3").is_some());
    assert!(sockets == 1);
    assert!(cores_per_socket == 96);
    assert!(threads_per_core == 1);
}
