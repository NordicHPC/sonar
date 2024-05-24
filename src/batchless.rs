// jobs::JobManager for systems without a batch job queue.
//
// Since 4.3BSD it's been the case that "job" === "a process group", and POSIX defines it thus.
// Hence the process group ID of a process is its job ID, in the absence of other information.

use crate::jobs;
#[cfg(test)]
use crate::jobs::JobManager;
use crate::procfs;
use std::collections::HashMap;

pub struct BatchlessJobManager {}

impl BatchlessJobManager {
    pub fn new() -> BatchlessJobManager {
        BatchlessJobManager {}
    }
}

impl jobs::JobManager for BatchlessJobManager {
    fn job_id_from_pid(
        &mut self,
        proc_pid: usize,
        processes: &HashMap<usize, procfs::Process>,
    ) -> usize {
        if let Some(p) = processes.get(&proc_pid) {
            p.pgrp
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
    assert!(jm.job_id_from_pid(205415, &procs) == 205408);
    assert!(jm.job_id_from_pid(200, &procs) == 0); // lost process
}

// More data than we need right now, but oh well.
// ps -x -h -o pid,ppid,pgrp,cmd | awk '{ print "(" $1 ", " $2 ", " $3 ", " "\"" $4 "\")," }'
#[cfg(test)]
fn parsed_full_test_output() -> HashMap<usize, procfs::Process> {
    vec![
        (205060, 1, 205060, "/usr/lib/systemd/systemd"),
        (205074, 205060, 205060, "(sd-pam)"),
        (205095, 1, 205094, "/usr/bin/gnome-keyring-daemon"),
        (205120, 205004, 205120, "/usr/libexec/gdm-wayland-session"),
        (205129, 205060, 205129, "/usr/bin/dbus-broker-launch"),
        (205131, 205129, 205129, "dbus-broker"),
        (205135, 205120, 205120, "/usr/libexec/gnome-session-binary"),
        (205225, 205060, 205225, "/usr/libexec/gnome-session-ctl"),
        (205228, 205060, 205228, "/usr/libexec/uresourced"),
        (205231, 205060, 205231, "/usr/libexec/gnome-session-binary"),
        (205236, 205060, 205236, "/usr/libexec/gvfsd"),
        (205245, 205060, 205236, "/usr/libexec/gvfsd-fuse"),
        (205259, 205060, 205259, "/usr/bin/pipewire"),
        (205261, 205060, 205261, "/usr/bin/wireplumber"),
        (205310, 205060, 205310, "/usr/bin/gnome-shell"),
        (205408, 205060, 205408, "/usr/libexec/at-spi-bus-launcher"),
        (205414, 205408, 205408, "/usr/bin/dbus-broker-launch"),
        (205415, 205414, 205408, "dbus-broker"),
        (205417, 205060, 205417, "/usr/libexec/at-spi2-registryd"),
        (205426, 205060, 205426, "/usr/libexec/gnome-shell-calendar-server"),
        (205435, 205060, 205435, "/usr/libexec/xdg-permission-store"),
        (205440, 205060, 205440, "/usr/libexec/dconf-service"),
        (205451, 205060, 205451, "/usr/libexec/evolution-source-registry"),
        (205465, 205060, 205465, "/usr/bin/gjs"),
        (205470, 205060, 205470, "/usr/bin/ibus-daemon"),
        (205471, 205060, 205471, "/usr/libexec/gsd-a11y-settings"),
        (205477, 205060, 205477, "/usr/libexec/gsd-color"),
        (205479, 205060, 205479, "/usr/libexec/gsd-datetime"),
        (205487, 205060, 205487, "/usr/libexec/gsd-housekeeping"),
        (205490, 205060, 205490, "/usr/libexec/gsd-keyboard"),
        (205495, 205060, 205495, "/usr/libexec/gsd-media-keys"),
        (205512, 205231, 205231, "/usr/bin/gnome-software"),
        (205515, 205060, 205515, "/usr/libexec/gsd-power"),
        (205520, 205060, 205520, "/usr/libexec/gsd-print-notifications"),
        (205527, 205060, 205527, "/usr/libexec/gsd-rfkill"),
        (205530, 205060, 205530, "/usr/libexec/gsd-screensaver-proxy"),
        (205532, 205060, 205532, "/usr/libexec/gsd-sharing"),
        (205533, 205231, 205231, "/usr/libexec/gsd-disk-utility-notify"),
        (205545, 205060, 205545, "/usr/libexec/gsd-smartcard"),
        (205560, 205060, 205560, "/usr/libexec/gsd-sound"),
        (205567, 205231, 205231, "/usr/libexec/evolution-data-server/evolution-alarm-notify"),
        (205569, 205060, 205569, "/usr/libexec/gsd-usb-protection"),
        (205578, 205060, 205578, "/usr/libexec/gsd-wacom"),
        (205652, 205470, 205470, "/usr/libexec/ibus-dconf"),
        (205653, 205470, 205470, "/usr/libexec/ibus-extension-gtk3"),
        (205677, 205060, 205677, "/usr/libexec/ibus-portal"),
        (205678, 205060, 205678, "/usr/bin/abrt-applet"),
        (205679, 205060, 205679, "/usr/libexec/goa-daemon"),
        (205683, 205060, 205683, "/usr/bin/gjs"),
        (205687, 205060, 205687, "/usr/libexec/evolution-calendar-factory"),
        (205688, 205060, 205688, "/usr/libexec/gvfs-udisks2-volume-monitor"),
        (205730, 205060, 205730, "/usr/libexec/gvfs-mtp-volume-monitor"),
        (205747, 205060, 205747, "/usr/libexec/goa-identity-service"),
        (205750, 205060, 205750, "/usr/libexec/evolution-addressbook-factory"),
        (205761, 205060, 205761, "/usr/libexec/gvfs-gphoto2-volume-monitor"),
        (205778, 205060, 205778, "/usr/libexec/gvfs-goa-volume-monitor"),
        (205792, 205060, 205792, "/usr/libexec/gvfs-afc-volume-monitor"),
        (205834, 205060, 205520, "/usr/libexec/gsd-printer"),
        (205855, 205470, 205470, "/usr/libexec/ibus-engine-simple"),
        (205868, 205060, 205868, "/usr/bin/pipewire-pulse"),
        (205898, 205060, 205898, "/usr/libexec/xdg-desktop-portal"),
        (205916, 205060, 205916, "/usr/libexec/xdg-document-portal"),
        (205947, 205060, 205947, "/usr/libexec/xdg-desktop-portal-gnome"),
        (206013, 205060, 206013, "/usr/libexec/xdg-desktop-portal-gtk"),
        (206065, 205060, 206065, "/usr/libexec/gvfsd-metadata"),
        (206124, 205310, 205310, "/usr/lib64/firefox/firefox"),
        (206145, 205060, 206145, "/usr/libexec/cgroupify"),
        (206230, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206252, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206278, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206327, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206331, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206334, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206346, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206353, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206374, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206396, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206421, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206442, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206619, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206624, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206641, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (206785, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (207040, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (207088, 205470, 205470, "/usr/bin/python3"),
        (207209, 205060, 207209, "/usr/libexec/gnome-terminal-server"),
        (207228, 207209, 207228, "bash"),
        (207437, 205310, 205310, "/usr/bin/emacs"),
        (207440, 205310, 205310, "/usr/bin/Xwayland"),
        (207445, 205060, 207445, "/usr/libexec/gsd-xsettings"),
        (207463, 205060, 207445, "/usr/libexec/ibus-x11"),
        (207476, 205310, 205310, "/usr/libexec/mutter-x11-frames"),
        (207591, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (208077, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (208121, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (211068, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (211133, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (211281, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (211375, 206124, 205310, "/usr/lib64/firefox/firefox"),
        (211404, 207228, 211404, "ps"),
        (211405, 207228, 211404, "awk"),
    ]
    .iter()
    .map(|(pid, ppid, pgrp, command)| {
        (
            *pid,
            procfs::Process {
                pid: *pid,
                ppid: *ppid,
                pgrp: *pgrp,
                command: command.to_string(),
                // The following are wrong but we don't need them now
                cpu_pct: 0.0,
                cputime_sec: 0,
                mem_pct: 0.0,
                mem_size_kib: 0,
                rssanon_kib: 0,
                uid: 0,
                user: "user".to_string(),
                has_children: false,
            },
        )
    })
    .collect::<HashMap<usize, procfs::Process>>()
}
