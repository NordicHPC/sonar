// jobs::JobManager for systems without a job queue.
//
// In this system, the job ID of a process is found by walking the tree of jobs from the PID until
// we reach a process that is directly below a session leader, and then taking that process's PID as
// the job ID.  (The session leader is a process whose session PID is its own PID.)  If the process
// we're given is a session leader then we take its own PID to be the job ID.  Most other things end
// up at the init process or at "the system", which is OK - their own PIDs are their job IDs.
//
// There's a possibility that the job ID will be reused during the lifetime of the system, confusing
// our statistics.  On Linux, the PIDs wrap around at about 4e6, and on a busy system this happens
// in a matter of days.  However, the job ID is not used in isolation (at the moment), but always
// with the user and the command name, so the reuse problem is not huge.
//
// There's also a challenge with this scheme in that, since the output is keyed on program name as
// well as on user name and job ID, multiple output lines are going to have the same user name and
// job ID in a tree of *different* processes (ie where subprocesses of the root process in the job
// exec something with a different name).  This is not wrong but it is something that the consumer
// must take into account.  For example, in assessing the resources for a job, the resources for all
// the different programs for the job must be taken into account.

use crate::jobs;
#[cfg(test)]
use crate::jobs::JobManager;
use crate::procfs;
use std::collections::HashMap;

pub struct BatchlessJobManager {
    // Process tables can be large and searching them sequentially for every lookup will be slow, so
    // add a cache.  Various structures could work.  Here a hashmap maps PID -> (session, PPID).
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
    fn lookup(&mut self, processes: &[procfs::Process], want_pid: usize) -> Option<(usize, usize)> {
        let probe = self.cache.get(&want_pid);
        if probe.is_some() {
            return probe.copied();
        }

        for procfs::Process {
            pid, ppid, session, ..
        } in processes
        {
            if *pid == want_pid {
                let entry = (*session, *ppid);
                self.cache.insert(want_pid, entry);
                return Some(entry);
            }
        }

        None
    }
}

impl jobs::JobManager for BatchlessJobManager {
    fn job_id_from_pid(&mut self, mut proc_pid: usize, processes: &[procfs::Process]) -> usize {
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
}

#[test]
fn test_batchless_jobs() {
    let mut jm = BatchlessJobManager::new();
    let procs = parsed_full_test_output();
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

// Generated from an old PS output and subsequently munged.
#[cfg(test)]
fn parsed_full_test_output() -> Vec<procfs::Process> {
    vec![
        (1, "systemd", 0, 1),
        (2, "kthreadd", 0, 0),
        (3, "rcu_gp", 2, 0),
        (4, "rcu_par_gp", 2, 0),
        (5, "slub_flushwq", 2, 0),
        (6, "netns", 2, 0),
        (8, "kworker/0:0H-events_highpri", 2, 0),
        (10, "mm_percpu_wq", 2, 0),
        (11, "rcu_tasks_kthread", 2, 0),
        (12, "rcu_tasks_rude_kthread", 2, 0),
        (13, "rcu_tasks_trace_kthread", 2, 0),
        (14, "ksoftirqd/0", 2, 0),
        (15, "rcu_preempt", 2, 0),
        (16, "migration/0", 2, 0),
        (17, "idle_inject/0", 2, 0),
        (19, "cpuhp/0", 2, 0),
        (20, "cpuhp/1", 2, 0),
        (21, "idle_inject/1", 2, 0),
        (22, "migration/1", 2, 0),
        (23, "ksoftirqd/1", 2, 0),
        (25, "kworker/1:0H-events_highpri", 2, 0),
        (26, "cpuhp/2", 2, 0),
        (27, "idle_inject/2", 2, 0),
        (28, "migration/2", 2, 0),
        (29, "ksoftirqd/2", 2, 0),
        (31, "kworker/2:0H-events_highpri", 2, 0),
        (32, "cpuhp/3", 2, 0),
        (33, "idle_inject/3", 2, 0),
        (34, "migration/3", 2, 0),
        (35, "ksoftirqd/3", 2, 0),
        (37, "kworker/3:0H-events_highpri", 2, 0),
        (38, "cpuhp/4", 2, 0),
        (39, "idle_inject/4", 2, 0),
        (40, "migration/4", 2, 0),
        (41, "ksoftirqd/4", 2, 0),
        (43, "kworker/4:0H-kblockd", 2, 0),
        (44, "cpuhp/5", 2, 0),
        (45, "idle_inject/5", 2, 0),
        (46, "migration/5", 2, 0),
        (47, "ksoftirqd/5", 2, 0),
        (49, "kworker/5:0H-events_highpri", 2, 0),
        (50, "cpuhp/6", 2, 0),
        (51, "idle_inject/6", 2, 0),
        (52, "migration/6", 2, 0),
        (53, "ksoftirqd/6", 2, 0),
        (55, "kworker/6:0H-events_highpri", 2, 0),
        (56, "cpuhp/7", 2, 0),
        (57, "idle_inject/7", 2, 0),
        (58, "migration/7", 2, 0),
        (59, "ksoftirqd/7", 2, 0),
        (61, "kworker/7:0H-events_highpri", 2, 0),
        (62, "kdevtmpfs", 2, 0),
        (63, "inet_frag_wq", 2, 0),
        (64, "kauditd", 2, 0),
        (65, "khungtaskd", 2, 0),
        (67, "oom_reaper", 2, 0),
        (69, "writeback", 2, 0),
        (70, "kcompactd0", 2, 0),
        (71, "ksmd", 2, 0),
        (72, "khugepaged", 2, 0),
        (73, "kintegrityd", 2, 0),
        (74, "kblockd", 2, 0),
        (75, "blkcg_punt_bio", 2, 0),
        (78, "tpm_dev_wq", 2, 0),
        (79, "ata_sff", 2, 0),
        (81, "md", 2, 0),
        (82, "edac-poller", 2, 0),
        (83, "devfreq_wq", 2, 0),
        (84, "watchdogd", 2, 0),
        (85, "kworker/0:1H-acpi_thermal_pm", 2, 0),
        (86, "kswapd0", 2, 0),
        (87, "ecryptfs-kthread", 2, 0),
        (93, "kthrotld", 2, 0),
        (98, "irq/124-pciehp", 2, 0),
        (99, "irq/125-pciehp", 2, 0),
        (104, "acpi_thermal_pm", 2, 0),
        (105, "xenbus_probe", 2, 0),
        (107, "vfio-irqfd-clea", 2, 0),
        (108, "mld", 2, 0),
        (109, "kworker/5:1H-kblockd", 2, 0),
        (110, "ipv6_addrconf", 2, 0),
        (115, "kstrp", 2, 0),
        (121, "zswap-shrink", 2, 0),
        (170, "charger_manager", 2, 0),
        (208, "kworker/7:1H-events_highpri", 2, 0),
        (229, "kworker/3:1H-events_highpri", 2, 0),
        (231, "nvme-wq", 2, 0),
        (232, "nvme-reset-wq", 2, 0),
        (233, "nvme-delete-wq", 2, 0),
        (238, "irq/173-SYNA30B7:00", 2, 0),
        (239, "kworker/2:1H-events_highpri", 2, 0),
        (243, "irq/174-WACF4233:00", 2, 0),
        (267, "jbd2/nvme0n1p2-8", 2, 0),
        (268, "ext4-rsv-conver", 2, 0),
        (303, "kworker/6:1H-kblockd", 2, 0),
        (308, "systemd-journal", 1, 308),
        (335, "kworker/4:1H-events_highpri", 2, 0),
        (336, "kworker/1:1H-events_highpri", 2, 0),
        (339, "systemd-udevd", 1, 339),
        (469, "cfg80211", 2, 0),
        (485, "irq/175-iwlwifi:default_queue", 2, 0),
        (488, "irq/176-iwlwifi:queue_1", 2, 0),
        (489, "irq/177-iwlwifi:queue_2", 2, 0),
        (490, "irq/178-iwlwifi:queue_3", 2, 0),
        (491, "irq/179-iwlwifi:queue_4", 2, 0),
        (492, "irq/180-iwlwifi:queue_5", 2, 0),
        (493, "irq/181-iwlwifi:queue_6", 2, 0),
        (494, "irq/182-iwlwifi:queue_7", 2, 0),
        (496, "irq/183-iwlwifi:queue_8", 2, 0),
        (498, "irq/184-iwlwifi:exception", 2, 0),
        (512, "systemd-oomd", 1, 512),
        (513, "systemd-resolve", 1, 513),
        (514, "systemd-timesyn", 1, 514),
        (535, "cryptd", 2, 0),
        (581, "accounts-daemon", 1, 581),
        (584, "acpid", 1, 584),
        (587, "avahi-daemon", 1, 587),
        (589, "cron", 1, 589),
        (590, "dbus-daemon", 1, 590),
        (592, "NetworkManager", 1, 592),
        (602, "irqbalance", 1, 602),
        (616, "networkd-dispat", 1, 616),
        (617, "polkitd", 1, 617),
        (618, "power-profiles-", 1, 618),
        (619, "rsyslogd", 1, 619),
        (621, "snapd", 1, 621),
        (626, "switcheroo-cont", 1, 626),
        (643, "systemd-logind", 1, 643),
        (654, "thermald", 1, 654),
        (655, "udisksd", 1, 655),
        (677, "wpa_supplicant", 1, 677),
        (687, "avahi-daemon", 587, 587),
        (719, "ModemManager", 1, 719),
        (722, "boltd", 1, 722),
        (751, "unattended-upgr", 1, 751),
        (757, "gdm3", 1, 757),
        (761, "iio-sensor-prox", 1, 761),
        (792, "bluetoothd", 1, 792),
        (799, "card0-crtc0", 2, 0),
        (800, "card0-crtc1", 2, 0),
        (801, "card0-crtc2", 2, 0),
        (802, "card0-crtc3", 2, 0),
        (960, "irq/207-AudioDSP", 2, 0),
        (1079, "rtkit-daemon", 1, 1079),
        (1088, "upowerd", 1, 1088),
        (1352, "packagekitd", 1, 1352),
        (1523, "colord", 1, 1523),
        (1618, "kerneloops", 1, 1618),
        (1622, "kerneloops", 1, 1622),
        (1789, "gdm-session-wor", 757, 757),
        (1804, "systemd", 1, 1804),
        (1805, "(sd-pam)", 1804, 1804),
        (1811, "pipewire", 1804, 1811),
        (1812, "pipewire-media-", 1804, 1812),
        (1813, "pulseaudio", 1804, 1813),
        (1823, "dbus-daemon", 1804, 1823),
        (1825, "gnome-keyring-d", 1, 1824),
        (1834, "gvfsd", 1804, 1834),
        (1840, "gvfsd-fuse", 1804, 1834),
        (1855, "xdg-document-po", 1804, 1855),
        (1859, "xdg-permission-", 1804, 1859),
        (1865, "fusermount3", 1855, 1865),
        (1884, "tracker-miner-f", 1804, 1884),
        (1892, "krfcommd", 2, 0),
        (1894, "gvfs-udisks2-vo", 1804, 1894),
        (1899, "gvfs-mtp-volume", 1804, 1899),
        (1903, "gvfs-goa-volume", 1804, 1903),
        (1907, "goa-daemon", 1804, 1823),
        (1914, "goa-identity-se", 1804, 1823),
        (1916, "gvfs-afc-volume", 1804, 1916),
        (1925, "gvfs-gphoto2-vo", 1804, 1925),
        (1938, "gdm-wayland-ses", 1789, 1938),
        (1943, "gnome-session-b", 1938, 1938),
        (1985, "gnome-session-c", 1804, 1985),
        (1997, "gnome-session-b", 1804, 1997),
        (2019, "gnome-shell", 1804, 2019),
        (2020, "at-spi-bus-laun", 1997, 1997),
        (2028, "dbus-daemon", 2020, 1997),
        (2136, "gvfsd-metadata", 1804, 2136),
        (2144, "gnome-shell-cal", 1804, 1823),
        (2150, "evolution-sourc", 1804, 2150),
        (2163, "dconf-service", 1804, 2163),
        (2168, "evolution-calen", 1804, 2168),
        (2183, "evolution-addre", 1804, 2183),
        (2198, "gjs", 1804, 1823),
        (2200, "at-spi2-registr", 1804, 1997),
        (2208, "gvfsd-trash", 1834, 1834),
        (2222, "sh", 1804, 2222),
        (2223, "gsd-a11y-settin", 1804, 2223),
        (2225, "ibus-daemon", 2222, 2222),
        (2226, "gsd-color", 1804, 2226),
        (2229, "gsd-datetime", 1804, 2229),
        (2231, "gsd-housekeepin", 1804, 2231),
        (2232, "gsd-keyboard", 1804, 2232),
        (2233, "gsd-media-keys", 1804, 2233),
        (2234, "gsd-power", 1804, 2234),
        (2236, "gsd-print-notif", 1804, 2236),
        (2238, "gsd-rfkill", 1804, 2238),
        (2239, "gsd-screensaver", 1804, 2239),
        (2240, "gsd-sharing", 1804, 2240),
        (2241, "gsd-smartcard", 1804, 2241),
        (2242, "gsd-sound", 1804, 2242),
        (2243, "gsd-wacom", 1804, 2243),
        (2303, "ibus-memconf", 2225, 2222),
        (2305, "ibus-extension-", 2225, 2222),
        (2308, "ibus-portal", 1804, 1823),
        (2311, "evolution-alarm", 1997, 1997),
        (2319, "gsd-disk-utilit", 1997, 1997),
        (2375, "snap-store", 1804, 1997),
        (2417, "ibus-engine-sim", 2225, 2222),
        (2465, "gsd-printer", 1804, 2236),
        (2520, "xdg-desktop-por", 1804, 2520),
        (2530, "xdg-desktop-por", 1804, 2530),
        (2555, "gjs", 1804, 1823),
        (2573, "xdg-desktop-por", 1804, 2573),
        (2636, "fwupd", 1, 2636),
        (2656, "snapd-desktop-i", 1804, 2656),
        (2734, "snapd-desktop-i", 2656, 2656),
        (3325, "Xwayland", 2019, 2019),
        (3344, "gsd-xsettings", 1804, 3344),
        (3375, "ibus-x11", 1804, 3344),
        (3884, "snap", 1804, 1823),
        (5131, "update-notifier", 1997, 1997),
        (7780, "gvfsd-http", 1834, 1834),
        (9221, "gnome-terminal-", 1804, 9221),
        (9239, "bash", 9221, 9239),
        (11438, "obsidian", 2019, 2019),
        (11495, "obsidian", 11438, 2019),
        (11496, "obsidian", 11438, 2019),
        (11526, "obsidian", 11495, 2019),
        (11531, "obsidian", 11438, 2019),
        (11542, "obsidian", 11438, 2019),
        (11543, "obsidian", 11438, 2019),
        (12887, "ssh-agent", 1825, 1824),
        (74536, "bash", 9221, 74536),
        (80195, "gnome-calendar", 1804, 1823),
        (80199, "seahorse", 200, 1823),
        (82329, "firefox", 2019, 2019),
        (82497, "Socket procfs::Process", 82329, 2019),
        (82516, "Privileged Cont", 82329, 2019),
        (82554, "Isolated Web Co", 82329, 2019),
        (82558, "Isolated Web Co", 82329, 2019),
        (82562, "Isolated Web Co", 82329, 2019),
        (82572, "Isolated Web Co", 82329, 2019),
        (82584, "Isolated Web Co", 82329, 2019),
        (82605, "Isolated Web Co", 82329, 2019),
        (82631, "Isolated Web Co", 82329, 2019),
        (82652, "Isolated Web Co", 82329, 2019),
        (82680, "Isolated Web Co", 82329, 2019),
        (82732, "Isolated Web Co", 82329, 2019),
        (83002, "WebExtensions", 82329, 2019),
        (83286, "Isolated Web Co", 82329, 2019),
        (83326, "Isolated Web Co", 82329, 2019),
        (83332, "RDD procfs::Process", 82329, 2019),
        (83340, "Utility procfs::Process", 82329, 2019),
        (83618, "Isolated Web Co", 82329, 2019),
        (83689, "Isolated Web Co", 82329, 2019),
        (83925, "Isolated Web Co", 82329, 2019),
        (84013, "Isolated Web Co", 82329, 2019),
        (84177, "Isolated Web Co", 82329, 2019),
        (96883, "Isolated Web Co", 82329, 2019),
        (97718, "Isolated Web Co", 82329, 2019),
        (99395, "Isolated Web Co", 82329, 2019),
        (99587, "Isolated Web Co", 82329, 2019),
        (103356, "Isolated Web Co", 82329, 2019),
        (103359, "Isolated Web Co", 82329, 2019),
        (103470, "file:// Content", 82329, 2019),
        (104433, "Isolated Web Co", 82329, 2019),
        (104953, "Isolated Web Co", 82329, 2019),
        (116260, "Isolated Web Co", 82329, 2019),
        (116296, "Isolated Web Co", 82329, 2019),
        (116609, "Isolated Web Co", 82329, 2019),
        (116645, "Isolated Web Co", 82329, 2019),
        (116675, "Isolated Web Co", 82329, 2019),
        (116997, "Isolated Web Co", 82329, 2019),
        (119104, "Isolated Web Co", 82329, 2019),
        (119151, "Isolated Web Co", 82329, 2019),
        (128778, "emacs", 2019, 2019),
        (132391, "Isolated Web Co", 82329, 2019),
        (133097, "Isolated Web Co", 82329, 2019),
        (134154, "Isolated Web Co", 82329, 2019),
        (135609, "Isolated Web Co", 82329, 2019),
        (136169, "kworker/u17:1-i915_flip", 2, 0),
        (140722, "Isolated Web Co", 82329, 2019),
        (142642, "kworker/u17:0-i915_flip", 2, 0),
        (144346, "kworker/1:1-events", 2, 0),
        (144602, "kworker/u16:57-events_unbound", 2, 0),
        (144609, "kworker/u16:64-events_power_efficient", 2, 0),
        (144624, "irq/185-mei_me", 2, 0),
        (144736, "cupsd", 1, 144736),
        (144754, "cups-browsed", 1, 144754),
        (145490, "gjs", 2019, 2019),
        (145716, "kworker/7:2-events", 2, 0),
        (146289, "kworker/u16:0-events_power_efficient", 2, 0),
        (146290, "kworker/6:1-events", 2, 0),
        (146342, "kworker/2:1-events", 2, 0),
        (146384, "kworker/5:0-events", 2, 0),
        (146735, "kworker/0:0-events", 2, 0),
        (146791, "kworker/1:2-events", 2, 0),
        (147017, "kworker/4:2-events", 2, 0),
        (147313, "kworker/3:2-events", 2, 0),
        (147413, "kworker/7:0-mm_percpu_wq", 2, 0),
        (147421, "kworker/6:2-inet_frag_wq", 2, 0),
        (147709, "kworker/2:2-events", 2, 0),
        (147914, "kworker/5:2-events", 2, 0),
        (147916, "kworker/4:0-events", 2, 0),
        (147954, "kworker/1:3-mm_percpu_wq", 2, 0),
        (148064, "kworker/3:0-events", 2, 0),
        (148065, "kworker/0:2-events", 2, 0),
        (148141, "kworker/7:1-events", 2, 0),
        (148142, "kworker/u17:2", 2, 0),
        (148173, "kworker/6:0-events", 2, 0),
        (148253, "kworker/2:0", 2, 0),
        (148259, "Isolated Servic", 82329, 2019),
        (148284, "kworker/u16:1-events_power_efficient", 2, 0),
        (148286, "kworker/4:1-events_freezable", 2, 0),
        (148299, "Web Content", 82329, 2019),
        (148301, "Web Content", 82329, 2019),
        (148367, "kworker/3:1-events", 2, 0),
        (148371, "kworker/5:1-events", 2, 0),
        (148378, "Web Content", 82329, 2019),
        (148406, "ps", 9239, 9239),
    ]
    .iter()
    .map(|(pid, command, ppid, session)| procfs::Process {
        pid: *pid,
        command: command.to_string(),
        ppid: *ppid,
        session: *session,
        cpu_pct: 0.0,
        cputime_sec: 0,
        mem_pct: 0.0,
        mem_size_kib: 0,
        rssanon_kib: 0,
        uid: 0,
        user: "user".to_string(),
    })
    .collect::<Vec<procfs::Process>>()
}
