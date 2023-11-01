/// Collect CPU process information without GPU information, from files in /proc.

use crate::process;
use crate::procfsapi;

use std::collections::HashMap;

/// Obtain process information via /proc and return a vector of structures with all the information
/// we need.  In the returned vector, pids uniquely tag the records.
///
/// This returns Ok(data) on success, otherwise Err(msg), and in the latter case the caller should
/// fallback to running `ps` and consider logging msg.
///
/// This function uniformly uses /proc, even though in some cases there are system calls that
/// provide the same information.
///
/// The underlying computing system -- /proc, system tables, and clock -- is virtualized through the
/// ProcfsAPI instance.

pub fn get_process_information(fs: &dyn procfsapi::ProcfsAPI) -> Result<Vec<process::Process>, String> {
    // The boot time is the `btime` field of /proc/stat.  It is measured in seconds since epoch.  We
    // need this to compute the process's real time, which we need to compute ps-compatible cpu
    // utilization.

    let mut boot_time = 0;
    let stat_s = fs.read_to_string("stat")?;
    for l in stat_s.split('\n') {
        if l.starts_with("btime ") {
            let fields = l.split_ascii_whitespace().collect::<Vec<&str>>();
            boot_time = parse_usize_field(&fields, 1, l, "stat", 0, "btime")? as u64;
            break;
        }
    }
    if boot_time == 0 {
        return Err(format!("Could not find btime in /proc/stat: {stat_s}"));
    }

    // The total RAM installed is in the `MemTotal` field of /proc/meminfo.  We need this to compute
    // ps-compatible relative memory use.

    let mut memtotal_kib = 0;
    let meminfo_s = fs.read_to_string("meminfo")?;
    for l in meminfo_s.split('\n') {
        if l.starts_with("MemTotal: ") {
            // We expect "MemTotal:\s+(\d+)\s+kB", roughly
            let fields = l.split_ascii_whitespace().collect::<Vec<&str>>();
            if fields.len() != 3 || fields[2] != "kB" {
                return Err(format!("Unexpected MemTotal in /proc/meminfo: {l}"));
            }
            memtotal_kib = parse_usize_field(&fields, 1, l, "meminfo", 0, "MemTotal")?;
            break;
        }
    }
    if memtotal_kib == 0 {
        return Err(format!(
            "Could not find MemTotal in /proc/meminfo: {meminfo_s}"
        ));
    }

    // Enumerate all pids, and collect the uids while we're here.
    //
    // Just ignore dirents that cause trouble, there wouldn't normally be any in proc, but if there
    // are we probably don't care.  We assume that sonar has sufficient permissions to inspect all
    // "interesting" processes.
    //
    // Note that a pid may disappear between the time we see it here and the time we get around to
    // reading it, later, and that new pids may appear meanwhile.  We should ignore both issues.

    let pids = fs.read_proc_pids()?;

    // Collect remaining system data from /proc/{pid}/stat for the enumerated pids.

    let kib_per_page = fs.page_size_in_kib();
    let mut result = vec![];
    let mut user_table = UserTable::new();
    let clock_ticks_per_sec = fs.clock_ticks_per_sec() as f64;
    if clock_ticks_per_sec == 0.0 {
        return Err("Could not get a sensible CLK_TCK".to_string());
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
        if let Ok(line) = fs.read_to_string(&format!("{pid}/stat")) {
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

            // NOTE relative to the `proc` documentation: All field offsets in the following are
            // relative to the command (so ppid is 2, not 4), and then they are zero-based, not
            // 1-based (so ppid is actually 1).

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
            // In particular for Z, field 5 "tpgid" has been observed to be -1.  For X, many of the
            // fields are -1.

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
                comm += " <defunct>";
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
            let now_ticks = fs.now_in_secs_since_epoch() as f64 * clock_ticks_per_sec;
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
        if let Ok(s) = fs.read_to_string(&format!("{pid}/statm")) {
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

        // clock_ticks_per_sec is nonzero, so this division will not produce NaN or Infinity
        let cputime_sec = (bsdtime_ticks / clock_ticks_per_sec).round() as usize;

        // Note ps uses rss not size here.  Also, ps doesn't trust rss to be <= 100% of memory, so
        // let's not trust it either.  memtotal_kib is nonzero, so this division will not produce
        // NaN or Infinity.
        let pmem = f64::min(
            ((rss_kib as f64) * 1000.0 / (memtotal_kib as f64)).round() / 10.0,
            99.9,
        );

        result.push(process::Process {
            pid,
            uid: uid as usize,
            user: user_table.lookup(fs, uid),
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
    ht: HashMap<u32, String>,
}

impl UserTable {
    fn new() -> UserTable {
        UserTable { ht: HashMap::new() }
    }

    fn lookup(&mut self, fs: &dyn procfsapi::ProcfsAPI, uid: u32) -> String {
        if let Some(name) = self.ht.get(&uid) {
            name.clone()
        } else if let Some(name) = fs.user_by_uid(uid) {
            self.ht.insert(uid, name.clone());
            name
        } else {
            "_noinfo_".to_string()
        }
    }
}

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
        "cpu  241155 582 127006 12838870 12445 0 3816 0 0 0
cpu0 32528 189 19573 1597325 1493 0 1149 0 0 0
cpu1 32258 98 17128 1597900 1618 0 550 0 0 0
cpu2 30018 18 13638 1607769 1565 0 340 0 0 0
cpu3 31888 23 16103 1603771 1663 0 217 0 0 0
cpu4 32830 54 27843 1581301 1506 0 295 0 0 0
cpu5 27206 111 10254 1618633 1509 0 325 0 0 0
cpu6 26842 26 9906 1619446 1514 0 511 0 0 0
cpu7 27582 61 12558 1612723 1575 0 426 0 0 0
intr 24686011 0 9 0 0 0 0 0 0 0 46121 0 0 0 0 2 0 0 0 0 0 0 0 0 0 0 0 660271 642 0 0 0 0 0 0 0 0 0 0 0 0 1016 0 0 0 0 0 0 0 3 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 120 122 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 155340 23 18728 14408 20861 13683 16444 17251 14218 17364 1 1 107 159457 6997 9903 12495 7135 5125 5225 7316 7414 3 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1414903 2183 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
ctxt 51751779
btime 1698303295
processes 30162
procs_running 1
procs_blocked 0
softirq 14448542 1561885 1201818 5 226550 10931 0 58705 8120272 39 3268337".to_string());
    files.insert(
        "meminfo".to_string(),
        "MemTotal:       16093776 kB
MemFree:         5247088 kB
MemAvailable:    8162068 kB
Buffers:          203244 kB
Cached:          3999448 kB
SwapCached:            0 kB
Active:          1405072 kB
Inactive:        7805220 kB
Active(anon):       6808 kB
Inactive(anon):  6112636 kB
Active(file):    1398264 kB
Inactive(file):  1692584 kB
Unevictable:      982716 kB
Mlocked:              16 kB
SwapTotal:       2097148 kB
SwapFree:        2097148 kB
Zswap:                 0 kB
Zswapped:              0 kB
Dirty:              2872 kB
Writeback:             0 kB
AnonPages:       5990404 kB
Mapped:           672068 kB
Shmem:           1111828 kB
KReclaimable:     168520 kB
Slab:             385396 kB
SReclaimable:     168520 kB
SUnreclaim:       216876 kB
KernelStack:       29632 kB
PageTables:        66172 kB
SecPageTables:         0 kB
NFS_Unstable:          0 kB
Bounce:                0 kB
WritebackTmp:          0 kB
CommitLimit:    10144036 kB
Committed_AS:   16010888 kB
VmallocTotal:   34359738367 kB
VmallocUsed:       68332 kB
VmallocChunk:          0 kB
Percpu:             8160 kB
HardwareCorrupted:     0 kB
AnonHugePages:         0 kB
ShmemHugePages:   890880 kB
ShmemPmdMapped:        0 kB
FileHugePages:         0 kB
FilePmdMapped:         0 kB
HugePages_Total:       0
HugePages_Free:        0
HugePages_Rsvd:        0
HugePages_Surp:        0
Hugepagesize:       2048 kB
Hugetlb:               0 kB
DirectMap4k:      254828 kB
DirectMap2M:     4710400 kB
DirectMap1G:    11534336 kB
"
        .to_string(),
    );
    files.insert(
        "4018/stat".to_string(),
        "4018 (firefox) S 2190 2189 2189 0 -1 4194560 19293188 3117638 1823 557 51361 15728 5390 2925 20 0 187 0 16400 5144358912 184775 18446744073709551615 94466859782144 94466860597976 140720852341888 0 0 0 0 4096 17663 0 0 0 17 4 0 0 0 0 0 94466860605280 94466860610840 94466863497216 140720852350777 140720852350820 140720852350820 140720852357069 0".to_string());
    files.insert(
        "4018/statm".to_string(),
        "1255967 185959 54972 200 0 316078 0".to_string(),
    );

    let ticks_per_sec = 100.0;     // We define this
    let utime_ticks = 51361.0;     // field(/proc/4018/stat, 14)
    let stime_ticks = 15728.0;     // field(/proc/4018/stat, 15)
    let boot_time = 1698303295.0;  // field(/proc/stat, "btime")
    let start_ticks = 16400.0;     // field(/proc/4018/stat, 22)
    let rss: f64 = 185959.0 * 4.0; // pages_to_kib(field(/proc/4018/statm, 1))
    let memtotal = 16093776.0;     // field(/proc/meminfo, "MemTotal:")
    let size = 316078 * 4;         // pages_to_kib(field(/proc/4018/statm, 5))

    // now = boot_time + start_time + utime_ticks + stime_ticks + arbitrary idle time
    let now = (boot_time + (start_ticks / ticks_per_sec) + (utime_ticks / ticks_per_sec) + (stime_ticks / ticks_per_sec) + 2000.0) as u64;

    let fs = procfsapi::MockFS::new(files, pids, users, now);
    let info = get_process_information(&fs).unwrap();
    assert!(info.len() == 1);
    let p = &info[0];
    assert!(p.pid == 4018);     // from enumeration of /proc
    assert!(p.uid == 1000);     // ditto
    assert!(p.user == "zappa"); // from getent
    assert!(p.command == "firefox"); // field(/proc/4018/stat, 2)
    assert!(p.ppid == 2190);    // field(/proc/4018/stat, 4)
    assert!(p.session == 2189); // field(/proc/4018/stat, 6)

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
}

#[test]
pub fn procfs_dead_and_undead_test() {
    let pids = vec![(4018, 1000), (4019, 1000), (4020, 1000)];

    let mut users = HashMap::new();
    users.insert(1000, "zappa".to_string());

    let mut files = HashMap::new();
    files.insert("stat".to_string(), "btime 1698303295".to_string());
    files.insert("meminfo".to_string(), "MemTotal:       16093776 kB".to_string());
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

    let fs = procfsapi::MockFS::new(files, pids, users, procfsapi::unix_now());
    let info = get_process_information(&fs).unwrap();

    // 4020 should be dropped - it's dead
    assert!(info.len() == 2);

    assert!(info[0].pid == 4018);
    assert!(info[0].command == "firefox");
    assert!(info[1].pid == 4019);
    assert!(info[1].command == "firefox <defunct>");
}
