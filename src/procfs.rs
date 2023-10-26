/// Collect CPU process information without GPU information, from files in /proc.
extern crate libc;
extern crate page_size;
extern crate users;

use crate::process;

use std::collections::HashMap;
use std::fs;
use std::os::linux::fs::MetadataExt;
use std::path;
use std::time::{SystemTime, UNIX_EPOCH};
use users::{get_user_by_uid, uid_t};

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

    // Values in "ticks" are represented as f64 here.  A typical value for CLK_TCK in 2023 is 100
    // (checked on several different systems).  There are about 2^23 ticks per day.  2^52/2^23=29,
    // ie 2^29 days, which is about 1.47 million years, without losing any precision.  Since we're
    // only ever running sonar on a single node, we will not exceed that range.

    let kib_per_page = page_size::get() / 1024;
    let mut result = vec![];
    let mut user_table = UserTable::new();
    let clock_ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) as f64 };
    if clock_ticks_per_sec == 0.0 {
        return Err(format!("Could not get a sensible CLK_TCK"));
    }

    for (pid, uid) in pids {
        // Basic system variables.  Intermediate time values are represented in ticks to prevent
        // various roundoff artifacts resulting in NaN or Infinity.

        let bsdtime_ticks;
        let mut realtime_ticks;
        let ppid;
        let sess;
        let mut comm;
        let utime_ticks;
        let stime_ticks;
        if let Ok(line) = fs::read_to_string(path::Path::new(&format!("/proc/{pid}/stat"))) {
            // The comm field is a little tricky, it must be extracted first as the contents between
            // the first '(' and the last ')' in the line.
            let commstart = line.find('(');
            let commend = line.rfind(')');
            if commstart.is_none() || commend.is_none() {
                return Err(format!(
                    "Could not parse command from /proc/{pid}/stat: {line}"
                ));
            }
            comm = line[commstart.unwrap() + 1..commend.unwrap()].to_string();
            let s = line[commend.unwrap() + 1..].trim().to_string();
            let fields = s.split_ascii_whitespace().collect::<Vec<&str>>();
            // NOTE relative to the `proc` documentation: All field offsets here are relative to the
            // command, so ppid is 2, not 4, and then they are zero-based, not 1-based.

            // Fields[0] is the state.  These characters are relevant for modern kernels:
            //  R running
            //  S sleeping in interruptible wait
            //  D sleeping in uninterruptible disk wait
            //  Z zombie
            //  T stopped on a signal
            //  t stopped for tracing
            //  X dead
            //
            // For some of these, the fields may take on different values, in particular, for X and
            // Z it is known that some of the values we want are represented as -1 and parsing them
            // as unsigned will fail.  For Z it also looks like some of the fields could have
            // surprising zero values; one has to be careful when dividing.
            //
            // In particular for Z, field 5 "tpgid" has been observed to be -1.

            // Zombie jobs cannot be ignored, because they are indicative of system health and the
            // information about their presence is used in consumers.

            let dead = fields[0] == "X";
            let zombie = fields[0] == "Z";

            if dead {
                // Just drop dead jobs
                continue;
            }

            if zombie {
                // This tag is used by consumers but it's an artifact of `ps`, not the kernel
                comm = comm + " <defunct>";
            }

            ppid = parse_usize_field(&fields, 1, &line, "stat", pid, "ppid")?;
            sess = parse_usize_field(&fields, 3, &line, "stat", pid, "sess")?;
            utime_ticks = parse_usize_field(&fields, 11, &line, "stat", pid, "utime")? as f64;
            stime_ticks = parse_usize_field(&fields, 12, &line, "stat", pid, "stime")? as f64;
            let cutime_ticks = parse_usize_field(&fields, 13, &line, "stat", pid, "cutime")? as f64;
            let cstime_ticks = parse_usize_field(&fields, 14, &line, "stat", pid, "cstime")? as f64;
            bsdtime_ticks = utime_ticks + stime_ticks + cutime_ticks + cstime_ticks;
            let start_time_ticks =
                parse_usize_field(&fields, 19, &line, "stat", pid, "starttime")? as f64;

            // boot_time and the current time are both time_t, ie, a 31-bit quantity in 2023 and a
            // 32-bit quantity before 2038.  clock_ticks_per_sec is on the order of 100.  Ergo
            // boot_ticks and now_ticks can be represented in about 32+7=39 bits, fine for an f64.
            let now_ticks = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as f64
                * clock_ticks_per_sec;
            let boot_ticks = boot_time as f64 * clock_ticks_per_sec;

            // start_time_ticks should be on the order of a few years, there is no risk of overflow
            // here, and in any case boot_ticks + start_time_ticks <= now_ticks, and by the above
            // reasoning now_ticks fits in an f64, ergo the sum does too.
            //
            // Take the max with 1 here to ensure realtime_ticks is not zero.
            realtime_ticks = now_ticks - (boot_ticks + start_time_ticks);
            if realtime_ticks < 1.0 {
                realtime_ticks = 1.0;
            }
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
            rss_kib = parse_usize_field(&fields, 1, &s, "statm", pid, "resident set size")?
                * kib_per_page;
            size_kib = parse_usize_field(&fields, 5, &s, "statm", pid, "data size")? * kib_per_page;
        } else {
            // This is *usually* benign - see above.
            continue;
        }

        // Now compute some derived quantities.

        // pcpu and pmem are rounded to ##.#.  We're going to get slightly different answers here
        // than ps because we use float arithmetic; frequently this code will produce values that
        // are one-tenth of a percent off from those of ps.  One can argue about whether round(),
        // floor() or ceil() is the most correct, but it's unlikely to matter much.

        // realtime_ticks is nonzero, so this division will not produce NaN or Infinity
        let pcpu_value = (utime_ticks + stime_ticks) / realtime_ticks;
        let pcpu_formatted = (pcpu_value * 1000.0).round() / 10.0;

        // clock_ticks_per_sec is nonzero, so these divisions will not produce NaN or Infinity
        let cputime_sec = (bsdtime_ticks / clock_ticks_per_sec).round() as usize;

        // Note ps uses rss not size here.  Also, ps doesn't trust rss to be <= 100% of memory, so
        // let's not trust it either.
        let pmem = f64::min(
            ((rss_kib as f64) * 1000.0 / (memtotal_kib as f64)).round() / 10.0,
            99.9,
        );

        let user = user_table.lookup(uid);

        result.push(process::Process {
            pid,
            uid: uid as usize,
            user,
            cpu_pct: pcpu_formatted,
            mem_pct: pmem,
            cputime_sec,
            mem_size_kib: size_kib,
            ppid,
            session: sess,
            command: comm,
        });
    }

    Ok(result)
}

fn parse_usize_field(
    fields: &[&str],
    ix: usize,
    line: &str,
    file: &str,
    pid: usize,
    fieldname: &str,
) -> Result<usize, String> {
    if ix >= fields.len() {
        if pid == 0 {
            return Err(format!("Index out of range for /proc/{file}: {ix}: {line}"));
        } else {
            return Err(format!(
                "Index out of range for /proc/{pid}/{file}: {ix}: {line}"
            ));
        }
    }
    if let Ok(n) = fields[ix].parse::<usize>() {
        return Ok(n);
    }
    if pid == 0 {
        Err(format!(
            "Could not parse {fieldname} in /proc/{file}: {line}"
        ))
    } else {
        Err(format!(
            "Could not parse {fieldname} from /proc/{pid}/{file}: {line}"
        ))
    }
}

// The UserTable optimizes uid -> name lookup.

struct UserTable {
    ht: HashMap<uid_t, String>,
}

impl UserTable {
    fn new() -> UserTable {
        UserTable { ht: HashMap::new() }
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
