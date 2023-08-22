use crate::process;

use std::fs;
use std::path;
use std::os::linux::fs::MetadataExt;

/// Obtain process information via /proc and return a vector of structures with all the information
/// we need.  In the returned vector, pids uniquely tag the records.
///
/// This returns Some(data) on success, otherwise None, and in the latter case the caller should
/// fallback to another method.

pub fn get_process_information() -> Option<Vec<process::Process>> {

    // Enumerate all pids first, and collect the uid while we're here.

    let mut pids = 
        if let Ok(dir) = fs::read_dir("/proc") {
            // FIXME: get rid of the unwraps
            dir
                .filter_map(|dirent| {
                    if let Ok(x) = dirent.as_ref().unwrap().path().to_str().unwrap().parse::<usize>() {
                        Some((x, dirent.unwrap().metadata().unwrap().st_uid()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<(usize, u32)>>()
        } else {
            return None;
        };
    
    let mut result = vec![];
    for (pid, uid) in pids {
        let mut pcpu;
        let mut pmem;
        let mut bsdtime;
        let mut ppid;
        let mut sess;
        let mut comm;

        // We want the values for the variables above.  These are obtainable from /proc/pid/stat.

        if let Ok(mut s) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/stat"))) {
            // The comm field is a little tricky, it must be extracted first as the contents between
            // the first '(' and the last ')' in the line.
            let commstart = s.find('(');
            let commend = s.rfind(')');
            if commstart.is_none() || commend.is_none() {
                return None;
            }
            comm = s[commstart.unwrap()..commend.unwrap()].to_string();
            s = s[commend.unwrap()..].to_string();
            let fields = s.split(' ').collect::<Vec<&str>>();
            if fields.len() < 16 {
                return None;
            }
            ppid = if let Ok(x) = fields[2].parse::<usize>() {
                x
            } else {
                return None;
            };
            sess = if let Ok(x) = fields[4].parse::<usize>() {
                x
            } else {
                return None;
            };
            bsdtime = if let Ok(cutime) = fields[14].parse::<usize>() {
                if let Ok(cstime) = fields[15].parse::<usize>() {
                    cutime + cstime
                } else {
                    return None;
                }
            } else {
                return None;
            };
        } else {
            eprintln!("Failed to open /proc/{pid}/stat");
            return None;
        }

        // We want the value corresponding to the "size" field printed by ps.  This is a saga.  When
        // ps prints "size", it is with the pr_swappable() function of procps-ng (flagged variously
        // in the source as "Ugly old Debian thing" and "SCO"). This prints VM_DATA + VM_STACK,
        // scaled to KiB.  The values for VM_DATA and VM_STACK are obtained from /proc/PID/status
        // [sic].  However the man page for /proc says that those values are inaccurate and that one
        // should instead use /proc/pid/statm.  In statm, we want the "data" field which is
        // documented as "data + stack", this is the sixth space-separated field.

        let mut size = 0;
        if let Ok(s) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/statm"))) {
            let fields = s.split(' ').collect::<Vec<&str>>();
            if fields.len() < 6 {
                return None;
            }
            size = if let Ok(x) = fields[5].parse::<usize>() {
                let pagesize = 4096; // FIXME
                x * pagesize / 1024
            } else {
                return None;
            };
        } else {
            eprintln!("Failed to open /proc/{pid}/statm");
            return None;
        }

        result.push(process::Process {
            pid,
            uid: uid as usize,
            user: "".to_string(), // User name is obtained later
            cpu_pct: pcpu as f64,
            mem_pct: pmem as f64,
            cputime_sec: bsdtime,
            mem_size_kib: size,
            ppid,
            session: sess,
            command: comm
        });
    }        

    Some(result)
}
