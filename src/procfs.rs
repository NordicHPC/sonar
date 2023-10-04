/// Collect CPU process information without GPU information, from files in /proc.

extern crate libc;
extern crate page_size;
extern crate users;

use crate::process;

use std::collections::HashMap;
use std::fs;
use std::path;
use std::os::linux::fs::MetadataExt;
use std::time::{SystemTime, UNIX_EPOCH};
use users::{uid_t, get_user_by_uid};

/// Obtain process information via /proc and return a vector of structures with all the information
/// we need.  In the returned vector, pids uniquely tag the records.
///
/// This returns Ok(data) on success, otherwise Err(msg), and in the latter case the caller should
/// fallback to running `ps` and consider logging msg.
///
/// This function uniformly uses /proc, even though in some cases there are system calls that
/// provide the same information.

pub fn get_process_information() -> Result<Vec<process::Process>, String> {

    // The boot time is the `btime` field of /proc/stat.  It is measured in seconds since epoch.  We
    // need this to compute the process's real time, which we need to compute ps-compatible cpu
    // utilization.

    let mut boot_time = 0;
    if let Ok(s) = fs::read_to_string(path::Path::new("/proc/stat")) {
        for l in s.split('\n') {
            if l.starts_with("btime ") {
                let fields = l.split_ascii_whitespace().collect::<Vec<&str>>();
                boot_time = parse_usize_field(&fields, 1, &l, "stat", 0, "btime")? as u64;
                break;
            }
        }
        if boot_time == 0 {
            return Err(format!("Could not find btime in /proc/stat: {s}"));
        }
    } else {
        return Err(format!("Could not open or read /proc/stat"));
    };

    // The total RAM installed is in the `MemTotal` field of /proc/meminfo.  We need this to compute
    // ps-compatible relative memory use.

    let mut memtotal_kib = 0;
    if let Ok(s) = fs::read_to_string(path::Path::new("/proc/meminfo")) {
        for l in s.split('\n') {
            if l.starts_with("MemTotal: ") {
                // We expect "MemTotal:\s+(\d+)\s+kB", roughly
                let fields = l.split_ascii_whitespace().collect::<Vec<&str>>();
                if fields.len() != 3 || fields[2] != "kB" {
                    return Err(format!("Unexpected MemTotal in /proc/meminfo: {l}"));
                }
                memtotal_kib = parse_usize_field(&fields, 1, &l, "meminfo", 0, "MemTotal")?;
                break;
            }
        }
        if memtotal_kib == 0 {
            return Err(format!("Could not find MemTotal in /proc/meminfo: {s}"));
        }
    } else {
        return Err(format!("Could not open or read /proc/meminfo"));
    };

    // Enumerate all pids, and collect the uids while we're here.
    //
    // Just ignore dirents that cause trouble, there wouldn't normally be any in proc, but if there
    // are we probably don't care.  We assume that sonar has sufficient permissions to inspect all
    // "interesting" processes.
    //
    // Note that a pid may disappear between the time we see it here and the time we get around to
    // reading it, later, and that new pids may appear meanwhile.  We should ignore both issues.

    let mut pids = vec![];
    if let Ok(dir) = fs::read_dir("/proc") {
        for dirent in dir {
            if let Ok(dirent) = dirent {
                if let Ok(meta) = dirent.metadata() {
                    let uid = meta.st_uid();
                    if let Some(name) = dirent.path().file_name() {
                        if let Ok(pid) = name.to_string_lossy().parse::<usize>() {
                            pids.push((pid, uid));
                        }
                    }
                }
            }
        }
    } else {
        return Err(format!("Could not open /proc"));
    };

    // Collect remaining system data from /proc/{pid}/stat for the enumerated pids.

    let kib_per_page = page_size::get() / 1024;
    let mut result = vec![];
    let mut user_table = UserTable::new();
    let clock_ticks_per_sec: usize = unsafe { libc::sysconf(libc::_SC_CLK_TCK) as usize };
    for (pid, uid) in pids {

        // Basic system variables.

        let bsdtime;
        let realtime;
        let ppid;
        let sess;
        let comm;
        let utime;
        let stime;
        if let Ok(line) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/stat"))) {
            // The comm field is a little tricky, it must be extracted first as the contents between
            // the first '(' and the last ')' in the line.
            let commstart = line.find('(');
            let commend = line.rfind(')');
            if commstart.is_none() || commend.is_none() {
                return Err(format!("Could not parse command from /proc/{pid}/stat: {line}"));
            }
            comm = line[commstart.unwrap()+1..commend.unwrap()].to_string();
            let s = line[commend.unwrap()+1..].trim().to_string();
            let fields = s.split_ascii_whitespace().collect::<Vec<&str>>();
            // NOTE relative to the `proc` documentation: All field offsets here are relative to the
            // command, so ppid is 2, not 4, and then they are zero-based, not 1-based.
            ppid = parse_usize_field(&fields, 1, &line, "stat", pid, "ppid")?;
            sess = parse_usize_field(&fields, 3, &line, "stat", pid, "sess")?;
            utime = parse_usize_field(&fields, 11, &line, "stat", pid, "utime")? / clock_ticks_per_sec;
            stime = parse_usize_field(&fields, 12, &line, "stat", pid, "stime")? / clock_ticks_per_sec;
            let cutime = parse_usize_field(&fields, 13, &line, "stat", pid, "cutime")? / clock_ticks_per_sec;
            let cstime = parse_usize_field(&fields, 14, &line, "stat", pid, "cstime")? / clock_ticks_per_sec;
            bsdtime = utime + stime + cutime + cstime;
            let start_time = (parse_usize_field(&fields, 19, &line, "stat", pid, "starttime")? / clock_ticks_per_sec) as u64;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            realtime = now - (boot_time + start_time);
        } else {
	    // This is *usually* benign - the process may have gone away since we enumerated the
	    // /proc directory.  It is *possibly* indicative of a permission problem, but that
	    // problem would be so pervasive that diagnosing it here is not right.
	    continue;
        }

        // We want the value corresponding to the "size" field printed by ps.  This is a saga.  When
        // ps prints "size", it is with the pr_swappable() function of procps-ng (flagged variously
        // in the source as "Ugly old Debian thing" and "SCO"). This prints VM_DATA + VM_STACK,
        // scaled to KiB.  The values for VM_DATA and VM_STACK are obtained from /proc/{pid}/status
        // (not /proc/{pid}/stat).  However the man page for /proc says that those values are
        // inaccurate and that one should instead use /proc/{pid}/statm.  In that file, we want the
        // "data" field which is documented as "data + stack", this is the sixth space-separated
        // field.

        let size_kib;
        let rss_kib;
        if let Ok(s) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/statm"))) {
            let fields = s.split_ascii_whitespace().collect::<Vec<&str>>();
            rss_kib = parse_usize_field(&fields, 1, &s, "statm", pid, "resident set size")? * kib_per_page;
            size_kib = parse_usize_field(&fields, 5, &s, "statm", pid, "data size")? * kib_per_page;
        } else {
	    // This is *usually* benign - see above.
	    continue;
        }

        // Now compute some derived quantities.

        // pcpu and pmem are rounded to ##.#.  We're going to get slightly different answers here
        // than ps because we use float arithmetic; frequently this code will produce values that
        // are one-tenth of a percent higher than ps.
        //
        // Note ps uses rss not size here.  Also, ps doesn't trust rss to be <= 100% of memory, so
        // let's not trust it either.

        let pcpu = (((utime + stime) as f64 * 1000.0 / realtime as f64)).ceil() / 10.0;
        let pmem = f64::min(((rss_kib as f64) * 1000.0 / (memtotal_kib as f64)).ceil() / 10.0, 99.9);
        let user = user_table.lookup(uid);

        result.push(process::Process {
            pid,
            uid: uid as usize,
            user,
            cpu_pct: pcpu,
            mem_pct: pmem,
            cputime_sec: bsdtime,
            mem_size_kib: size_kib,
            ppid,
            session: sess,
            command: comm
        });
    }

    Ok(result)
}

fn parse_usize_field(fields: &[&str], ix: usize, line: &str, file: &str, pid: usize, fieldname: &str) -> Result<usize, String> {
    if ix >= fields.len() {
        if pid == 0 {
            return Err(format!("Index out of range for /proc/{file}: {ix}: {line}"));
        } else {
            return Err(format!("Index out of range for /proc/{pid}/{file}: {ix}: {line}"));
        }
    }
    if let Ok(n) = fields[ix].parse::<usize>() {
        return Ok(n);
    }
    if pid == 0 {
        Err(format!("Could not parse {fieldname} in /proc/{file}: {line}"))
    } else {
        Err(format!("Could not parse {fieldname} from /proc/{pid}/{file}: {line}"))
    }
}

// The UserTable optimizes uid -> name lookup.

struct UserTable {
    ht: HashMap<uid_t, String>,
}

impl UserTable {
    fn new() -> UserTable {
        UserTable {
            ht: HashMap::new()
        }
    }

    fn lookup(&mut self, uid: uid_t) -> String {
        if let Some(name) = self.ht.get(&uid) {
            name.clone()
        } else if let Some(u) = get_user_by_uid(uid) {
            let name = u.name().to_string_lossy().to_string();
            self.ht.insert(uid, name.clone());
            name
        } else {
            "_noinfo_".to_string()
        }
    }
}
