// jobs::JobManager for systems without a batch job queue.
//
// Since 4.3BSD it's been the case that "job" === "a process group", and POSIX defines it thus.
// Hence the process group ID of a process is its job ID.  The process group ID is usually the
// process ID of the first process in the process group (and it's always the process ID of *some*
// process).
//
// Each job is a little tree of processes, and a job can and frequently will have subjobs.  For
// example, when the shell forks off a pipeline it puts the pipeline into a new process group.
// Shell job control works on the level of the process group.
//
// When a process creates a subprocess the subprocess will accumulate usage information, notably
// system and user time.  When the subprocess exits, and the parent process waits for it, this usage
// information is aggregated in the parent process as the "cutime" and "cstime" fields.  Thus when a
// job consists of multiple processes, the root process in the job will eventually have an
// accounting of all the time spent in the children in the job.
//
// This system is fairly reliable because processes almost always are created and destroyed in a
// tree-like fashion - data will bubble up toward the job root.  When Sonar observes process data,
// it will see this tree.
//
// This system breaks down however when a job creates a subjob, because the root process of the
// subjob is a subprocess of a process in the parent job.  The usage data for the subjob will be
// aggregated in the parent process of the subjob's root process, in the parent job.  We don't want
// that: the usage for each job should not affect that of other jobs.
//
// In order to fix this, the batchless job manager of Sonar will recreate the process and job tree,
// and, working bottom-up, will subtract job data for subjobs from parent jobs.
//
// ==> But oh, sonar is history-less, and this accounting only happens after a process has been
//     waited on, at which point we don't know the subjob data.
//
// The way to fix this is probably in sonalyze: for each process, log its job ID and its self time
// and the fact that it's batchless, and roll up the job when we have all the data for all the
// processes.  Always roll up "self" times, ignore child times - children will account for
// themselves.  This will strongly tend to underreport short-lived child jobs though.
//
//
//
//  : if A creates B which creates C, and B exits, B is kept around as a zombie until C exits,
// at which point data for B and C are aggregated into A.
//
// From the waitpid(2) man page:
//
//     A  child  that  terminates, but has not been waited for becomes a "zom‐
//     bie".  The kernel maintains a minimal set of information about the zom‐
//     bie process (PID, termination status, resource  usage  information)  in
//     order to allow the parent to later perform a wait to obtain information
//     about  the  child.   As long as a zombie is not removed from the system
//     via a wait, it will consume a slot in the kernel process table ....
//     If a parent process terminates, then its "zombie" children (if any) are
//     adopted by init(1), (or by the nearest "subreaper" process  as  defined
//     through  the  use  of  the  prctl(2) PR_SET_CHILD_SUBREAPER operation);
//     init(1) automatically performs a wait to remove the zombies.
//
//     POSIX.1-2001 specifies that if the disposition of  SIGCHLD  is  set  to
//     SIG_IGN or the SA_NOCLDWAIT flag is set for SIGCHLD (see sigaction(2)),
//     then children that terminate do not become zombies and a call to wait()
//     or  waitpid()  will  block until all children have terminated, and then
//     fail with errno set to ECHILD.
//
// (The normal disposition for SIGCHLD is SIG_IGN.)
//
// I think the upshot of this is that (a) *almost always* we can depend on a tree structure
// and (b) in the cases when there isn't a tree structure because the parent has exited, the
// usage data from the child will *not* percolate up our process tree, but will be lost in
// init, which is exactly what we want.

// To solve this problem,

// A process group leader L is always a subprocess of some P and thus normally L's CPU time is going
// to be accounted to the child time in P when P has wait()ed for L.  But this will confuse the
// statistics, and we must avoid that.  Any parent P of a process group leader L should therefore
// track only its own time, not the time of the job.  This gets hard when a parent has both job and
// non-job children: given data for P, we must subtract the data for the job children only.
//

//
// There's a possibility that the job ID will be reused during the lifetime of the system, confusing
// our statistics.  On Linux, the PIDs wrap around at about 4e6, and on a busy system this happens
// in a matter of days.  However, the job ID is not used in isolation (at the moment), but always
// with the user and the command name, so the reuse problem is not huge.
//
// There's also a challenge with this scheme in that, since the output is keyed on program name as
// well as on user name and job ID, multiple output lines are going to have the same user name and
// job ID in a tree of *different* processes (ie where subprocesses of the root process in the job
// `exec()` something with a different name).  This is not wrong but it is something that the
// consumer must take into account.  For example, in assessing the resources for a job, the
// resources for all the different programs for the job must be taken into account.

use crate::jobs;
#[cfg(test)]
use crate::jobs::JobManager;
use crate::procfs;
use std::collections::HashMap;

struct Procinfo {
    // the index of this process in the process table
    index: usize,

    // the index of the process's parent
    parent_index: Option<usize>,

    // the indices of the process's children
    child_indices: Vec<usize>,
}

pub struct BatchlessJobManager {
    procs: HashMap<usize, Procinfo>,
}

impl BatchlessJobManager {
    pub fn new() -> BatchlessJobManager {
        BatchlessJobManager {
            procs: HashMap::new(),
        }
    }
}

impl jobs::JobManager for BatchlessJobManager {
    // Preprocessing does two things:
    //
    //  - it populates self.procs so that we can quickly find process data by pid
    //  - it adjusts the cputime_sec field of processes whose child processes are process group
    //    leaders, so that the time from a job does not accrue to the parent process of the job.
    //
    fn preprocess(&mut self, mut processes: Vec<procfs::Process>) -> Vec<procfs::Process> {
        for (ix, proc) in processes.iter().enumerate() {
            self.procs.insert(proc.pid, Procinfo{
                index: ix,
                parent_index: None,
                child_indices: vec![],
            });
        }

        for (ix, proc) in processes.iter().enumerate() {
            let parent_ix =
                if let Some(parent) = self.procs.get_mut(&proc.ppid) {
                    parent.child_indices.push(ix);
                    Some(parent.index)
                } else {
                    None
                };
            self.procs.get_mut(&proc.pid).expect("No process!").parent_index = parent_ix;
        }

        // TODO: The "job ID" for a job should be the process group ID.  This does not really
        // reflect much of a change from the current setup but is More Correct.
        //
        // Note that *mostly* processes will be created and destroyed in a structured way, it is
        // rare that a parent exits before the child.  So *mostly* a traversal of the data with a
        // tree assumption will be just fine.
        //
        // If a parent exits before its child then there are special rules.  From the waitpid(2) man
        // page:
        /*
       A  child  that  terminates, but has not been waited for becomes a "zom‐
       bie".  The kernel maintains a minimal set of information about the zom‐
       bie process (PID, termination status, resource  usage  information)  in
       order to allow the parent to later perform a wait to obtain information
       about  the  child.   As long as a zombie is not removed from the system
       via a wait, it will consume a slot in the kernel process table, and  if
       this  table fills, it will not be possible to create further processes.
       If a parent process terminates, then its "zombie" children (if any) are
       adopted by init(1), (or by the nearest "subreaper" process  as  defined
       through  the  use  of  the  prctl(2) PR_SET_CHILD_SUBREAPER operation);
       init(1) automatically performs a wait to remove the zombies.

       POSIX.1-2001 specifies that if the disposition of  SIGCHLD  is  set  to
       SIG_IGN or the SA_NOCLDWAIT flag is set for SIGCHLD (see sigaction(2)),
       then children that terminate do not become zombies and a call to wait()
       or  waitpid()  will  block until all children have terminated, and then
       fail with errno set to ECHILD.
         */
        //
        // Note the normal disposition for SIGCHLD is SIG_IGN.
        //
        // I think the upshot of this is that (a) *almost always* we can depend on a tree structure
        // and (b) in the cases when there isn't a tree structure because the parent has exited, the
        // usage data from the child will *not* percolate up our process tree, but will be lost in
        // init, which is exactly what we want.



        // Adjust the cputime_sec field of the entries.
        //
        // This needs to be careful about ordering - it must proceed bottom-up.  Consider a case where
        // jobs are nested A - B - C.  B must be adjusted before A is adjusted.

        // When we subtract T from a process P and P is a child of a job A then we need to subtract T
        // from all the parents of A up to P.

        // In general, the two aspects fit together:
        //  - dfs-traverse the tree yielding a toposort
        //  - working from the rear, if a process is a pg leader then subtract its total time from all
        //    the process parents all the way to the top of its tree
        //  - there is going to be a problem with discontinuities in the tree if an intermediary process exits,
        //    sonar may not observe it

        // even dfs is suspect, there is not one root but many

        let mut i = 0;
        while i < processes.len() {
            let pid = processes[i].pid;
            let mut time = processes[i].cputime_sec;
            for c in self.procs.get(&pid).expect("No process!").child_indices.iter() {
                let child = &processes[*c];
                if child.pid == child.process_group {
                    time -= child.cputime_sec;
                }
            }
            processes[i].cputime_sec = time;
            i += 1;
        }

        processes
    }

    fn job_id_from_pid(&mut self, proc_pid: usize, processes: &[procfs::Process]) -> usize {
        if let Some(probe) = self.procs.get(&proc_pid) {
            processes[probe.index].process_group
        } else {
            // Lost process is job 0
            0
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

// FIXME: Get some new data!

#[cfg(test)]
fn parsed_full_test_output() -> Vec<procfs::Process> {
    vec![
        (1, "systemd", 0, 1, 0),
        (2, "kthreadd", 0, 0, 0),
        (3, "rcu_gp", 2, 0, 0),
        (4, "rcu_par_gp", 2, 0, 0),
        (5, "slub_flushwq", 2, 0, 0),
        (6, "netns", 2, 0, 0),
        (8, "kworker/0:0H-events_highpri", 2, 0, 0),
        (10, "mm_percpu_wq", 2, 0, 0),
        (11, "rcu_tasks_kthread", 2, 0, 0),
        (12, "rcu_tasks_rude_kthread", 2, 0, 0),
        (13, "rcu_tasks_trace_kthread", 2, 0, 0),
        (14, "ksoftirqd/0", 2, 0, 0),
        (15, "rcu_preempt", 2, 0, 0),
        (16, "migration/0", 2, 0, 0),
        (17, "idle_inject/0", 2, 0, 0),
        (19, "cpuhp/0", 2, 0, 0),
        (20, "cpuhp/1", 2, 0, 0),
        (21, "idle_inject/1", 2, 0, 0),
        (22, "migration/1", 2, 0, 0),
        (23, "ksoftirqd/1", 2, 0, 0),
        (25, "kworker/1:0H-events_highpri", 2, 0, 0),
        (26, "cpuhp/2", 2, 0, 0),
        (27, "idle_inject/2", 2, 0, 0),
        (28, "migration/2", 2, 0, 0),
        (29, "ksoftirqd/2", 2, 0, 0),
        (31, "kworker/2:0H-events_highpri", 2, 0, 0),
        (32, "cpuhp/3", 2, 0, 0),
        (33, "idle_inject/3", 2, 0, 0),
        (34, "migration/3", 2, 0, 0),
        (35, "ksoftirqd/3", 2, 0, 0),
        (37, "kworker/3:0H-events_highpri", 2, 0, 0),
        (38, "cpuhp/4", 2, 0, 0),
        (39, "idle_inject/4", 2, 0, 0),
        (40, "migration/4", 2, 0, 0),
        (41, "ksoftirqd/4", 2, 0, 0),
        (43, "kworker/4:0H-kblockd", 2, 0, 0),
        (44, "cpuhp/5", 2, 0, 0),
        (45, "idle_inject/5", 2, 0, 0),
        (46, "migration/5", 2, 0, 0),
        (47, "ksoftirqd/5", 2, 0, 0),
        (49, "kworker/5:0H-events_highpri", 2, 0, 0),
        (50, "cpuhp/6", 2, 0, 0),
        (51, "idle_inject/6", 2, 0, 0),
        (52, "migration/6", 2, 0, 0),
        (53, "ksoftirqd/6", 2, 0, 0),
        (55, "kworker/6:0H-events_highpri", 2, 0, 0),
        (56, "cpuhp/7", 2, 0, 0),
        (57, "idle_inject/7", 2, 0, 0),
        (58, "migration/7", 2, 0, 0),
        (59, "ksoftirqd/7", 2, 0, 0),
        (61, "kworker/7:0H-events_highpri", 2, 0, 0),
        (62, "kdevtmpfs", 2, 0, 0),
        (63, "inet_frag_wq", 2, 0, 0),
        (64, "kauditd", 2, 0, 0),
        (65, "khungtaskd", 2, 0, 0),
        (67, "oom_reaper", 2, 0, 0),
        (69, "writeback", 2, 0, 0),
        (70, "kcompactd0", 2, 0, 0),
        (71, "ksmd", 2, 0, 0),
        (72, "khugepaged", 2, 0, 0),
        (73, "kintegrityd", 2, 0, 0),
        (74, "kblockd", 2, 0, 0),
        (75, "blkcg_punt_bio", 2, 0, 0),
        (78, "tpm_dev_wq", 2, 0, 0),
        (79, "ata_sff", 2, 0, 0),
        (81, "md", 2, 0, 0),
        (82, "edac-poller", 2, 0, 0),
        (83, "devfreq_wq", 2, 0, 0),
        (84, "watchdogd", 2, 0, 0),
        (85, "kworker/0:1H-acpi_thermal_pm", 2, 0, 0),
        (86, "kswapd0", 2, 0, 0),
        (87, "ecryptfs-kthread", 2, 0, 0),
        (93, "kthrotld", 2, 0, 0),
        (98, "irq/124-pciehp", 2, 0, 0),
        (99, "irq/125-pciehp", 2, 0, 0),
        (104, "acpi_thermal_pm", 2, 0, 0),
        (105, "xenbus_probe", 2, 0, 0),
        (107, "vfio-irqfd-clea", 2, 0, 0),
        (108, "mld", 2, 0, 0),
        (109, "kworker/5:1H-kblockd", 2, 0, 0),
        (110, "ipv6_addrconf", 2, 0, 0),
        (115, "kstrp", 2, 0, 0),
        (121, "zswap-shrink", 2, 0, 0),
        (170, "charger_manager", 2, 0, 0),
        (208, "kworker/7:1H-events_highpri", 2, 0, 0),
        (229, "kworker/3:1H-events_highpri", 2, 0, 0),
        (231, "nvme-wq", 2, 0, 0),
        (232, "nvme-reset-wq", 2, 0, 0),
        (233, "nvme-delete-wq", 2, 0, 0),
        (238, "irq/173-SYNA30B7:00", 2, 0, 0),
        (239, "kworker/2:1H-events_highpri", 2, 0, 0),
        (243, "irq/174-WACF4233:00", 2, 0, 0),
        (267, "jbd2/nvme0n1p2-8", 2, 0, 0),
        (268, "ext4-rsv-conver", 2, 0, 0),
        (303, "kworker/6:1H-kblockd", 2, 0, 0),
        (308, "systemd-journal", 1, 308, 0),
        (335, "kworker/4:1H-events_highpri", 2, 0, 0),
        (336, "kworker/1:1H-events_highpri", 2, 0, 0),
        (339, "systemd-udevd", 1, 339, 0),
        (469, "cfg80211", 2, 0, 0),
        (485, "irq/175-iwlwifi:default_queue", 2, 0, 0),
        (488, "irq/176-iwlwifi:queue_1", 2, 0, 0),
        (489, "irq/177-iwlwifi:queue_2", 2, 0, 0),
        (490, "irq/178-iwlwifi:queue_3", 2, 0, 0),
        (491, "irq/179-iwlwifi:queue_4", 2, 0, 0),
        (492, "irq/180-iwlwifi:queue_5", 2, 0, 0),
        (493, "irq/181-iwlwifi:queue_6", 2, 0, 0),
        (494, "irq/182-iwlwifi:queue_7", 2, 0, 0),
        (496, "irq/183-iwlwifi:queue_8", 2, 0, 0),
        (498, "irq/184-iwlwifi:exception", 2, 0, 0),
        (512, "systemd-oomd", 1, 512, 0),
        (513, "systemd-resolve", 1, 513, 0),
        (514, "systemd-timesyn", 1, 514, 0),
        (535, "cryptd", 2, 0, 0),
        (581, "accounts-daemon", 1, 581, 0),
        (584, "acpid", 1, 584, 0),
        (587, "avahi-daemon", 1, 587, 0),
        (589, "cron", 1, 589, 0),
        (590, "dbus-daemon", 1, 590, 0),
        (592, "NetworkManager", 1, 592, 0),
        (602, "irqbalance", 1, 602, 0),
        (616, "networkd-dispat", 1, 616, 0),
        (617, "polkitd", 1, 617, 0),
        (618, "power-profiles-", 1, 618, 0),
        (619, "rsyslogd", 1, 619, 0),
        (621, "snapd", 1, 621, 0),
        (626, "switcheroo-cont", 1, 626, 0),
        (643, "systemd-logind", 1, 643, 0),
        (654, "thermald", 1, 654, 0),
        (655, "udisksd", 1, 655, 0),
        (677, "wpa_supplicant", 1, 677, 0),
        (687, "avahi-daemon", 587, 587, 0),
        (719, "ModemManager", 1, 719, 0),
        (722, "boltd", 1, 722, 0),
        (751, "unattended-upgr", 1, 751, 0),
        (757, "gdm3", 1, 757, 0),
        (761, "iio-sensor-prox", 1, 761, 0),
        (792, "bluetoothd", 1, 792, 0),
        (799, "card0-crtc0", 2, 0, 0),
        (800, "card0-crtc1", 2, 0, 0),
        (801, "card0-crtc2", 2, 0, 0),
        (802, "card0-crtc3", 2, 0, 0),
        (960, "irq/207-AudioDSP", 2, 0, 0),
        (1079, "rtkit-daemon", 1, 1079, 0),
        (1088, "upowerd", 1, 1088, 0),
        (1352, "packagekitd", 1, 1352, 0),
        (1523, "colord", 1, 1523, 0),
        (1618, "kerneloops", 1, 1618, 0),
        (1622, "kerneloops", 1, 1622, 0),
        (1789, "gdm-session-wor", 757, 757, 0),
        (1804, "systemd", 1, 1804, 0),
        (1805, "(sd-pam)", 1804, 1804, 0),
        (1811, "pipewire", 1804, 1811, 0),
        (1812, "pipewire-media-", 1804, 1812, 0),
        (1813, "pulseaudio", 1804, 1813, 0),
        (1823, "dbus-daemon", 1804, 1823, 0),
        (1825, "gnome-keyring-d", 1, 1824, 0),
        (1834, "gvfsd", 1804, 1834, 0),
        (1840, "gvfsd-fuse", 1804, 1834, 0),
        (1855, "xdg-document-po", 1804, 1855, 0),
        (1859, "xdg-permission-", 1804, 1859, 0),
        (1865, "fusermount3", 1855, 1865, 0),
        (1884, "tracker-miner-f", 1804, 1884, 0),
        (1892, "krfcommd", 2, 0, 0),
        (1894, "gvfs-udisks2-vo", 1804, 1894, 0),
        (1899, "gvfs-mtp-volume", 1804, 1899, 0),
        (1903, "gvfs-goa-volume", 1804, 1903, 0),
        (1907, "goa-daemon", 1804, 1823, 0),
        (1914, "goa-identity-se", 1804, 1823, 0),
        (1916, "gvfs-afc-volume", 1804, 1916, 0),
        (1925, "gvfs-gphoto2-vo", 1804, 1925, 0),
        (1938, "gdm-wayland-ses", 1789, 1938, 0),
        (1943, "gnome-session-b", 1938, 1938, 0),
        (1985, "gnome-session-c", 1804, 1985, 0),
        (1997, "gnome-session-b", 1804, 1997, 0),
        (2019, "gnome-shell", 1804, 2019, 0),
        (2020, "at-spi-bus-laun", 1997, 1997, 0),
        (2028, "dbus-daemon", 2020, 1997, 0),
        (2136, "gvfsd-metadata", 1804, 2136, 0),
        (2144, "gnome-shell-cal", 1804, 1823, 0),
        (2150, "evolution-sourc", 1804, 2150, 0),
        (2163, "dconf-service", 1804, 2163, 0),
        (2168, "evolution-calen", 1804, 2168, 0),
        (2183, "evolution-addre", 1804, 2183, 0),
        (2198, "gjs", 1804, 1823, 0),
        (2200, "at-spi2-registr", 1804, 1997, 0),
        (2208, "gvfsd-trash", 1834, 1834, 0),
        (2222, "sh", 1804, 2222, 0),
        (2223, "gsd-a11y-settin", 1804, 2223, 0),
        (2225, "ibus-daemon", 2222, 2222, 0),
        (2226, "gsd-color", 1804, 2226, 0),
        (2229, "gsd-datetime", 1804, 2229, 0),
        (2231, "gsd-housekeepin", 1804, 2231, 0),
        (2232, "gsd-keyboard", 1804, 2232, 0),
        (2233, "gsd-media-keys", 1804, 2233, 0),
        (2234, "gsd-power", 1804, 2234, 0),
        (2236, "gsd-print-notif", 1804, 2236, 0),
        (2238, "gsd-rfkill", 1804, 2238, 0),
        (2239, "gsd-screensaver", 1804, 2239, 0),
        (2240, "gsd-sharing", 1804, 2240, 0),
        (2241, "gsd-smartcard", 1804, 2241, 0),
        (2242, "gsd-sound", 1804, 2242, 0),
        (2243, "gsd-wacom", 1804, 2243, 0),
        (2303, "ibus-memconf", 2225, 2222, 0),
        (2305, "ibus-extension-", 2225, 2222, 0),
        (2308, "ibus-portal", 1804, 1823, 0),
        (2311, "evolution-alarm", 1997, 1997, 0),
        (2319, "gsd-disk-utilit", 1997, 1997, 0),
        (2375, "snap-store", 1804, 1997, 0),
        (2417, "ibus-engine-sim", 2225, 2222, 0),
        (2465, "gsd-printer", 1804, 2236, 0),
        (2520, "xdg-desktop-por", 1804, 2520, 0),
        (2530, "xdg-desktop-por", 1804, 2530, 0),
        (2555, "gjs", 1804, 1823, 0),
        (2573, "xdg-desktop-por", 1804, 2573, 0),
        (2636, "fwupd", 1, 2636, 0),
        (2656, "snapd-desktop-i", 1804, 2656, 0),
        (2734, "snapd-desktop-i", 2656, 2656, 0),
        (3325, "Xwayland", 2019, 2019, 0),
        (3344, "gsd-xsettings", 1804, 3344, 0),
        (3375, "ibus-x11", 1804, 3344, 0),
        (3884, "snap", 1804, 1823, 0),
        (5131, "update-notifier", 1997, 1997, 0),
        (7780, "gvfsd-http", 1834, 1834, 0),
        (9221, "gnome-terminal-", 1804, 9221, 0),
        (9239, "bash", 9221, 9239, 0),
        (11438, "obsidian", 2019, 2019, 0),
        (11495, "obsidian", 11438, 2019, 0),
        (11496, "obsidian", 11438, 2019, 0),
        (11526, "obsidian", 11495, 2019, 0),
        (11531, "obsidian", 11438, 2019, 0),
        (11542, "obsidian", 11438, 2019, 0),
        (11543, "obsidian", 11438, 2019, 0),
        (12887, "ssh-agent", 1825, 1824, 0),
        (74536, "bash", 9221, 74536, 0),
        (80195, "gnome-calendar", 1804, 1823, 0),
        (80199, "seahorse", 200, 1823, 0),
        (82329, "firefox", 2019, 2019, 0),
        (82497, "Socket procfs::Process", 82329, 2019, 0),
        (82516, "Privileged Cont", 82329, 2019, 0),
        (82554, "Isolated Web Co", 82329, 2019, 0),
        (82558, "Isolated Web Co", 82329, 2019, 0),
        (82562, "Isolated Web Co", 82329, 2019, 0),
        (82572, "Isolated Web Co", 82329, 2019, 0),
        (82584, "Isolated Web Co", 82329, 2019, 0),
        (82605, "Isolated Web Co", 82329, 2019, 0),
        (82631, "Isolated Web Co", 82329, 2019, 0),
        (82652, "Isolated Web Co", 82329, 2019, 0),
        (82680, "Isolated Web Co", 82329, 2019, 0),
        (82732, "Isolated Web Co", 82329, 2019, 0),
        (83002, "WebExtensions", 82329, 2019, 0),
        (83286, "Isolated Web Co", 82329, 2019, 0),
        (83326, "Isolated Web Co", 82329, 2019, 0),
        (83332, "RDD procfs::Process", 82329, 2019, 0),
        (83340, "Utility procfs::Process", 82329, 2019, 0),
        (83618, "Isolated Web Co", 82329, 2019, 0),
        (83689, "Isolated Web Co", 82329, 2019, 0),
        (83925, "Isolated Web Co", 82329, 2019, 0),
        (84013, "Isolated Web Co", 82329, 2019, 0),
        (84177, "Isolated Web Co", 82329, 2019, 0),
        (96883, "Isolated Web Co", 82329, 2019, 0),
        (97718, "Isolated Web Co", 82329, 2019, 0),
        (99395, "Isolated Web Co", 82329, 2019, 0),
        (99587, "Isolated Web Co", 82329, 2019, 0),
        (103356, "Isolated Web Co", 82329, 2019, 0),
        (103359, "Isolated Web Co", 82329, 2019, 0),
        (103470, "file:// Content", 82329, 2019, 0),
        (104433, "Isolated Web Co", 82329, 2019, 0),
        (104953, "Isolated Web Co", 82329, 2019, 0),
        (116260, "Isolated Web Co", 82329, 2019, 0),
        (116296, "Isolated Web Co", 82329, 2019, 0),
        (116609, "Isolated Web Co", 82329, 2019, 0),
        (116645, "Isolated Web Co", 82329, 2019, 0),
        (116675, "Isolated Web Co", 82329, 2019, 0),
        (116997, "Isolated Web Co", 82329, 2019, 0),
        (119104, "Isolated Web Co", 82329, 2019, 0),
        (119151, "Isolated Web Co", 82329, 2019, 0),
        (128778, "emacs", 2019, 2019, 0),
        (132391, "Isolated Web Co", 82329, 2019, 0),
        (133097, "Isolated Web Co", 82329, 2019, 0),
        (134154, "Isolated Web Co", 82329, 2019, 0),
        (135609, "Isolated Web Co", 82329, 2019, 0),
        (136169, "kworker/u17:1-i915_flip", 2, 0, 0),
        (140722, "Isolated Web Co", 82329, 2019, 0),
        (142642, "kworker/u17:0-i915_flip", 2, 0, 0),
        (144346, "kworker/1:1-events", 2, 0, 0),
        (144602, "kworker/u16:57-events_unbound", 2, 0, 0),
        (144609, "kworker/u16:64-events_power_efficient", 2, 0, 0),
        (144624, "irq/185-mei_me", 2, 0, 0),
        (144736, "cupsd", 1, 144736, 0),
        (144754, "cups-browsed", 1, 144754, 0),
        (145490, "gjs", 2019, 2019, 0),
        (145716, "kworker/7:2-events", 2, 0, 0),
        (146289, "kworker/u16:0-events_power_efficient", 2, 0, 0),
        (146290, "kworker/6:1-events", 2, 0, 0),
        (146342, "kworker/2:1-events", 2, 0, 0),
        (146384, "kworker/5:0-events", 2, 0, 0),
        (146735, "kworker/0:0-events", 2, 0, 0),
        (146791, "kworker/1:2-events", 2, 0, 0),
        (147017, "kworker/4:2-events", 2, 0, 0),
        (147313, "kworker/3:2-events", 2, 0, 0),
        (147413, "kworker/7:0-mm_percpu_wq", 2, 0, 0),
        (147421, "kworker/6:2-inet_frag_wq", 2, 0, 0),
        (147709, "kworker/2:2-events", 2, 0, 0),
        (147914, "kworker/5:2-events", 2, 0, 0),
        (147916, "kworker/4:0-events", 2, 0, 0),
        (147954, "kworker/1:3-mm_percpu_wq", 2, 0, 0),
        (148064, "kworker/3:0-events", 2, 0, 0),
        (148065, "kworker/0:2-events", 2, 0, 0),
        (148141, "kworker/7:1-events", 2, 0, 0),
        (148142, "kworker/u17:2", 2, 0, 0),
        (148173, "kworker/6:0-events", 2, 0, 0),
        (148253, "kworker/2:0", 2, 0, 0),
        (148259, "Isolated Servic", 82329, 2019, 0),
        (148284, "kworker/u16:1-events_power_efficient", 2, 0, 0),
        (148286, "kworker/4:1-events_freezable", 2, 0, 0),
        (148299, "Web Content", 82329, 2019, 0),
        (148301, "Web Content", 82329, 2019, 0),
        (148367, "kworker/3:1-events", 2, 0, 0),
        (148371, "kworker/5:1-events", 2, 0, 0),
        (148378, "Web Content", 82329, 2019, 0),
        (148406, "ps", 9239, 9239, 0),
    ]
    .iter()
    .map(|(pid, command, ppid, session, pgrp)| procfs::Process {
        pid: *pid,
        command: command.to_string(),
        ppid: *ppid,
        session: *session,
        process_group: *pgrp,
        cpu_pct: 0.0,
        cputime_sec: 0,
        childtime_sec: 0,
        mem_pct: 0.0,
        mem_size_kib: 0,
        rssanon_kib: 0,
        uid: 0,
        user: "user".to_string(),
    })
    .collect::<Vec<procfs::Process>>()
}
