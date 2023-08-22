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

    // The total RAM installed is in the `MemTotal` field of /proc/meminfo.

    let mut memtotal_kib = 0;
    if let Ok(s) = fs::read_to_string(path::Path::new("/proc/meminfo")) {
        for l in s.split('\n') {
            if l.starts_with("MemTotal: ") {
                // We expect "MemTotal:\s+(\d)+kB", roughly
                let fields = l.split_ascii_whitespace().collect::<Vec<&str>>();
                if fields.len() != 3 || fields[2] != "kB" {
                    eprintln!("Unexpected MemTotal in /proc/meminfo: {s}");
                    return None;
                }
                if let Ok(n) = fields[1].parse::<usize>() {
                    memtotal_kib = n;
                    break;
                } else {
                    eprintln!("Failed to parse MemTotal in /proc/meminfo: {s}");
                    return None;
                }
            }
        }
        if memtotal_kib == 0 {
            eprintln!("Could not find MemTotal in /proc/meminfo: {s}");
            return None;
        }
    } else {
        eprintln!("Could not open or read /proc/meminfo");
        return None;
    };

    // Enumerate all pids, and collect the uids while we're here.
    //
    // TODO: We can and should filter by uid here.

    let pids = 
        if let Ok(dir) = fs::read_dir("/proc") {
            // FIXME: get rid of the unwraps, and when we do so, there will be some error
            // propagation.
            dir
                .filter_map(|dirent| {
                    if let Ok(x) = dirent.as_ref().unwrap().path().file_name().unwrap().to_string_lossy().parse::<usize>() {
                        Some((x, dirent.unwrap().metadata().unwrap().st_uid()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<(usize, u32)>>()
        } else {
            eprintln!("Could not open /proc");
            return None;
        };
    
    let mut result = vec![];
    for (pid, uid) in pids {

        // Basic system variables.

        let bsdtime;
        let ppid;
        let sess;
        let comm;
        if let Ok(line) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/stat"))) {
            // The comm field is a little tricky, it must be extracted first as the contents between
            // the first '(' and the last ')' in the line.
            let commstart = line.find('(');
            let commend = line.rfind(')');
            if commstart.is_none() || commend.is_none() {
                eprintln!("Could not parse command from /proc/{pid}/stat: {line}");
                return None;
            }
            comm = line[commstart.unwrap()..commend.unwrap()].to_string();
            let s = line[commend.unwrap()..].to_string();
            let fields = s.split_ascii_whitespace().collect::<Vec<&str>>();
            if fields.len() < 16 {
                eprintln!("Line from /proc/{pid}/stat too short: {line}");
                return None;
            }
            ppid = if let Ok(x) = fields[2].parse::<usize>() {
                x
            } else {
                eprintln!("Could not parse ppid from /proc/{pid}/stat: {line}");
                return None;
            };
            sess = if let Ok(x) = fields[4].parse::<usize>() {
                x
            } else {
                eprintln!("Could not parse sess from /proc/{pid}/stat: {line}");
                return None;
            };
            bsdtime = if let Ok(cutime) = fields[14].parse::<usize>() {
                if let Ok(cstime) = fields[15].parse::<usize>() {
                    cutime + cstime
                } else {
                    eprintln!("Could not parse cstime from /proc/{pid}/stat: {line}");
                    return None;
                }
            } else {
                eprintln!("Could not parse cutime from /proc/{pid}/stat: {line}");
                return None;
            };
        } else {
            eprintln!("Failed to open or read /proc/{pid}/stat");
            return None;
        }

        // We want the value corresponding to the "size" field printed by ps.  This is a saga.  When
        // ps prints "size", it is with the pr_swappable() function of procps-ng (flagged variously
        // in the source as "Ugly old Debian thing" and "SCO"). This prints VM_DATA + VM_STACK,
        // scaled to KiB.  The values for VM_DATA and VM_STACK are obtained from /proc/PID/status
        // [sic].  However the man page for /proc says that those values are inaccurate and that one
        // should instead use /proc/pid/statm.  In statm, we want the "data" field which is
        // documented as "data + stack", this is the sixth space-separated field.

        let size;
        if let Ok(s) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/statm"))) {
            let fields = s.split_ascii_whitespace().collect::<Vec<&str>>();
            if fields.len() < 6 {
                eprintln!("Line from /proc/{pid}/statm too short: {s}");
                return None;
            }
            size = if let Ok(x) = fields[5].parse::<usize>() {
                let pagesize = 4096; // FIXME
                x * pagesize / 1024
            } else {
                eprintln!("Could not parse data size from /proc/{pid}/statm: {s}");
                return None;
            };
        } else {
            eprintln!("Failed to open /proc/{pid}/statm");
            return None;
        }

        // Now compute some derived quantities.  pcpu is becoming irrelevant, frankly.

        let pcpu = 0.0; // (now - starttime) / (cutime + cstime);
        let pmem = (size as f64) / (memtotal_kib as f64);

        result.push(process::Process {
            pid,
            uid: uid as usize,
            user: "".to_string(), // User name is obtained later
            cpu_pct: pcpu,
            mem_pct: pmem,
            cputime_sec: bsdtime,
            mem_size_kib: size,
            ppid,
            session: sess,
            command: comm
        });
    }        

    Some(result)
}
