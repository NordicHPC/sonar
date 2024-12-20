#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::gpu;
use crate::hostname;
use crate::interrupt;
use crate::jobs;
use crate::log;
use crate::procfs;
use crate::procfsapi;
use crate::util::{csv_quote, three_places};

use std::collections::{HashMap, HashSet};
use std::io::{self, Result, Write};
use std::path::PathBuf;

// The GpuSet has three states:
//
//  - the set is known to be empty, this is Some({})
//  - the set is known to be nonempty and have only known gpus in the set, this is Some({a,b,..})
//  - the set is known to be nonempty but have (some) unknown members, this is None
//
// During processing, the set starts out as Some({}).  If a device reports "unknown" GPUs then the
// set can transition from Some({}) to None or from Some({a,b,..}) to None.  Once in the None state,
// the set will stay in that state.  There is no representation for some known + some unknown GPUs,
// it is not believed to be worthwhile.

type GpuSet = Option<HashSet<usize>>;

fn empty_gpuset() -> GpuSet {
    Some(HashSet::new())
}

fn singleton_gpuset(maybe_device: Option<usize>) -> GpuSet {
    if let Some(dev) = maybe_device {
        let mut gpus = HashSet::new();
        gpus.insert(dev);
        Some(gpus)
    } else {
        None
    }
}

fn union_gpuset(lhs: &mut GpuSet, rhs: &GpuSet) {
    if lhs.is_none() {
        // The result is also None
    } else if rhs.is_none() {
        *lhs = None;
    } else {
        lhs.as_mut()
            .expect("LHS is nonempty")
            .extend(rhs.as_ref().expect("RHS is nonempty"));
    }
}

type Pid = usize;
type JobID = usize;

// ProcInfo holds per-process information gathered from multiple sources and tagged with a job ID.
// No processes are merged!  The job ID "0" means "unique job with no job ID".  That is, no consumer
// of this data, internal or external to the program, may treat separate processes with job ID "0"
// as part of the same job.

#[derive(Clone)]
struct ProcInfo<'a> {
    user: &'a str,
    _uid: usize,
    command: &'a str,
    pid: Pid,
    ppid: Pid,
    rolledup: usize,
    is_system_job: bool,
    has_children: bool,
    job_id: usize,
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_percentage: f64,
    mem_size_kib: usize,
    rssanon_kib: usize,
    gpu_cards: GpuSet,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size_kib: usize,
    gpu_status: GpuStatus,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum GpuStatus {
    Ok = 0,
    UnknownFailure = 1,
    // More here, by and by: it's possible to parse the output of the error and
    // be specific
}

type ProcTable<'a> = HashMap<Pid, ProcInfo<'a>>;

// The table mapping a Pid to user name / Uid is used by the GPU subsystems to provide information
// about users for the processes on the GPUS.

pub type Uid = usize;
pub type UserTable<'a> = HashMap<Pid, (&'a str, Uid)>;

// Add information about the process to the table `proc_by_pid`.  Here, `lookup_job_by_pid`, `user`,
// `command`, and `pid` must be provided while the subsequent fields are all optional and must be
// zero / empty if there's no information.

fn add_proc_info<'a, F>(
    proc_by_pid: &mut ProcTable<'a>,
    lookup_job_by_pid: &mut F,
    user: &'a str,
    uid: usize,
    command: &'a str,
    pid: Pid,
    ppid: Pid,
    has_children: bool,
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_percentage: f64,
    mem_size_kib: usize,
    rssanon_kib: usize,
    gpu_cards: &GpuSet,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size_kib: usize,
) where
    F: FnMut(Pid) -> JobID,
{
    proc_by_pid
        .entry(pid)
        .and_modify(|e| {
            // Already has user, command, pid, job_id
            e.cpu_percentage += cpu_percentage;
            e.cputime_sec += cputime_sec;
            e.mem_percentage += mem_percentage;
            e.mem_size_kib += mem_size_kib;
            e.rssanon_kib += rssanon_kib;
            union_gpuset(&mut e.gpu_cards, gpu_cards);
            e.gpu_percentage += gpu_percentage;
            e.gpu_mem_percentage += gpu_mem_percentage;
            e.gpu_mem_size_kib += gpu_mem_size_kib;
            assert!(has_children == e.has_children);
            assert!(ppid == e.ppid);
        })
        .or_insert(ProcInfo {
            user,
            _uid: uid,
            command,
            pid,
            ppid,
            rolledup: 0,
            is_system_job: uid < 1000,
            has_children,
            job_id: lookup_job_by_pid(pid),
            cpu_percentage,
            cputime_sec,
            mem_percentage,
            mem_size_kib,
            rssanon_kib,
            gpu_cards: gpu_cards.clone(),
            gpu_percentage,
            gpu_mem_percentage,
            gpu_mem_size_kib,
            gpu_status: GpuStatus::Ok,
        });
}

pub struct PsOptions<'a> {
    pub rollup: bool,
    pub always_print_something: bool,
    pub min_cpu_percent: Option<f64>,
    pub min_mem_percent: Option<f64>,
    pub min_cpu_time: Option<usize>,
    pub exclude_system_jobs: bool,
    pub exclude_users: Vec<&'a str>,
    pub exclude_commands: Vec<&'a str>,
    pub lockdir: Option<String>,
    pub load: bool,
}

pub fn create_snapshot(jobs: &mut dyn jobs::JobManager, opts: &PsOptions, timestamp: &str) {
    // If a lock file was requested, create one before the operation, exit early if it already
    // exists, and if we performed the operation, remove the file afterwards.  Otherwise, just
    // perform the operation.
    //
    // However if a signal arrives in the middle of the operation and terminates the program the
    // lock file may be left on disk.  Therefore some lightweight signal handling is desirable to
    // trap signals and clean up orderly.
    //
    // Additionally, if a signal is detected, we do not wish to start new operations, we can just
    // skip them.  Code therefore calls is_interrupted() at strategic points to check whether a
    // signal was detected.
    //
    // Finally, there's no reason to limit the signal handler to the case when we have a lock file,
    // the same logic can apply to both paths.

    interrupt::handle_interruptions();

    if let Some(ref dirname) = opts.lockdir {
        let mut created = false;
        let mut failed = false;
        let mut skip = false;
        let hostname = hostname::get();

        let mut p = PathBuf::new();
        p.push(dirname);
        p.push("sonar-lock.".to_string() + &hostname);

        if interrupt::is_interrupted() {
            return;
        }

        // create_new() requests atomic creation, if the file exists we'll error out.
        match std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&p)
        {
            Ok(mut f) => {
                created = true;
                let pid = std::process::id();
                match f.write(format!("{}", pid).as_bytes()) {
                    Ok(_) => {}
                    Err(_) => {
                        failed = true;
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                skip = true;
            }
            Err(_) => {
                failed = true;
            }
        }

        if !failed && !skip {
            do_create_snapshot(jobs, opts, timestamp);

            // Testing code: If we got the lockfile and produced a report, wait 10s after producing
            // it while holding onto the lockfile.  It is then possible to run sonar in that window
            // while the lockfile is being held, to ensure the second process exits immediately.
            #[cfg(debug_assertions)]
            if std::env::var("SONARTEST_WAIT_LOCKFILE").is_ok() {
                std::thread::sleep(std::time::Duration::new(10, 0));
            }
        }

        if created {
            match std::fs::remove_file(p) {
                Ok(_) => {}
                Err(_) => {
                    failed = true;
                }
            }
        }

        if skip {
            // Test cases depend on this exact message.
            log::info("Lockfile present, exiting");
        }
        if failed {
            log::error("Unable to properly manage or delete lockfile");
        }
    } else {
        do_create_snapshot(jobs, opts, timestamp);
    }
}

fn do_create_snapshot(jobs: &mut dyn jobs::JobManager, opts: &PsOptions, timestamp: &str) {
    let no_gpus = empty_gpuset();
    let mut proc_by_pid = ProcTable::new();

    if interrupt::is_interrupted() {
        return;
    }

    let fs = procfsapi::RealFS::new();

    // The total RAM installed is in the `MemTotal` field of /proc/meminfo.  We need this for
    // various things.  Not getting it is a hard error.

    let memtotal_kib = match procfs::get_memtotal_kib(&fs) {
        Ok(n) => n,
        Err(e) => {
            log::error(&format!("Could not get installed memory: {e}"));
            return;
        }
    };

    let (procinfo_output, _cpu_total_secs, per_cpu_secs) =
        match procfs::get_process_information(&fs, memtotal_kib) {
            Ok(result) => result,
            Err(msg) => {
                log::error(&format!("procfs failed: {msg}"));
                return;
            }
        };

    let pprocinfo_output = &procinfo_output;

    // The table of users is needed to get GPU information, see comments at UserTable.
    let mut user_by_pid = UserTable::new();
    for proc in pprocinfo_output.values() {
        user_by_pid.insert(proc.pid, (&proc.user, proc.uid));
    }

    let mut lookup_job_by_pid = |pid: Pid| jobs.job_id_from_pid(pid, pprocinfo_output);

    for proc in pprocinfo_output.values() {
        add_proc_info(
            &mut proc_by_pid,
            &mut lookup_job_by_pid,
            &proc.user,
            proc.uid,
            &proc.command,
            proc.pid,
            proc.ppid,
            proc.has_children,
            proc.cpu_pct,
            proc.cputime_sec,
            proc.mem_pct,
            proc.mem_size_kib,
            proc.rssanon_kib,
            &no_gpus, // gpu_cards
            0.0,      // gpu_percentage
            0.0,      // gpu_mem_percentage
            0,
        ); // gpu_mem_size_kib
    }

    if interrupt::is_interrupted() {
        return;
    }

    // When a GPU fails it may be a transient error or a permanent error, but either way sonar does
    // not know.  We just record the failure.
    //
    // This is a soft failure, surfaced through dashboards; we do not want mail about it under
    // normal circumstances.
    let mut gpu_status = GpuStatus::Ok;

    let gpu_utilization: Vec<gpu::Process>;
    let mut gpu_info: String = "".to_string();
    match gpu::probe() {
        None => {
            gpu_status = GpuStatus::UnknownFailure;
        }
        Some(mut gpu) => {
            match gpu.get_card_utilization() {
                Err(_) => {}
                Ok(ref cards) => {
                    let mut s = "".to_string();
                    s = add_key(s, "fan%", cards, |c: &gpu::CardState| {
                        nonzero(c.fan_speed_pct as i64)
                    });
                    s = add_key(s, "mode", cards, |c: &gpu::CardState| {
                        if c.compute_mode == "Default" {
                            "".to_string()
                        } else {
                            c.compute_mode.clone()
                        }
                    });
                    s = add_key(s, "perf", cards, |c: &gpu::CardState| c.perf_state.clone());
                    // Reserved memory is really not interesting, it's possible it would have been
                    // interesting as part of the card configuration.
                    //s = add_key(s, "mreskib", cards, |c: &gpu::CardState| nonzero(c.mem_reserved_kib));
                    s = add_key(s, "musekib", cards, |c: &gpu::CardState| {
                        nonzero(c.mem_used_kib)
                    });
                    s = add_key(s, "cutil%", cards, |c: &gpu::CardState| {
                        nonzero(c.gpu_utilization_pct as i64)
                    });
                    s = add_key(s, "mutil%", cards, |c: &gpu::CardState| {
                        nonzero(c.mem_utilization_pct as i64)
                    });
                    s = add_key(s, "tempc", cards, |c: &gpu::CardState| {
                        nonzero(c.temp_c.into())
                    });
                    s = add_key(s, "poww", cards, |c: &gpu::CardState| {
                        nonzero(c.power_watt.into())
                    });
                    s = add_key(s, "powlimw", cards, |c: &gpu::CardState| {
                        nonzero(c.power_limit_watt.into())
                    });
                    s = add_key(s, "cez", cards, |c: &gpu::CardState| {
                        nonzero(c.ce_clock_mhz.into())
                    });
                    s = add_key(s, "memz", cards, |c: &gpu::CardState| {
                        nonzero(c.mem_clock_mhz.into())
                    });
                    gpu_info = s;
                }
            }
            match gpu.get_process_utilization(&user_by_pid) {
                Err(_e) => {
                    gpu_status = GpuStatus::UnknownFailure;
                }
                Ok(conf) => {
                    gpu_utilization = conf;
                    for proc in &gpu_utilization {
                        let (ppid, has_children) =
                            if let Some(process) = pprocinfo_output.get(&proc.pid) {
                                (process.ppid, process.has_children)
                            } else {
                                (1, true)
                            };
                        add_proc_info(
                            &mut proc_by_pid,
                            &mut lookup_job_by_pid,
                            &proc.user,
                            proc.uid,
                            &proc.command,
                            proc.pid,
                            ppid,
                            has_children,
                            0.0, // cpu_percentage
                            0,   // cputime_sec
                            0.0, // mem_percentage
                            0,   // mem_size_kib
                            0,   // rssanon_kib
                            &singleton_gpuset(proc.device),
                            proc.gpu_pct,
                            proc.mem_pct,
                            proc.mem_size_kib,
                        );
                    }
                }
            }
        }
    }

    if interrupt::is_interrupted() {
        return;
    }

    // If there was a gpu failure, signal it in all the process structures.  This is pretty
    // conservative and increases data volume, but it means that the information is not lost so long
    // as not all records from this sonar run are filtered out by the front end.

    if gpu_status != GpuStatus::Ok {
        for proc_info in proc_by_pid.values_mut() {
            proc_info.gpu_status = gpu_status;
        }
    }

    if interrupt::is_interrupted() {
        return;
    }

    // Once we start printing we'll print everything and not check the interrupted flag any more.

    let mut writer = io::stdout();

    let hostname = hostname::get();
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let print_params = PrintParameters {
        hostname: &hostname,
        timestamp,
        version: VERSION,
        opts,
    };

    let mut candidates = if opts.rollup {
        // This is a little complicated because processes with job_id 0 or processes that have
        // subprocesses cannot be rolled up, nor can we roll up processes with different ppid.
        //
        // The reason we cannot roll up processes with job_id 0 is that we don't know that they are
        // related at all - 0 means "no information".
        //
        // The reason we cannot roll up processes with children or processes with different ppids is
        // that this would break subsequent processing - it would make it impossible to build a
        // sensible process tree from the sample data.
        //
        // - There is an array `rolledup` of ProcInfo nodes that represent rolled-up data
        //
        // - When the job ID of a process in `proc_by_pid` is zero, or a process has children, the
        //   entry in `rolledup` is a copy of that job
        //
        // - Otherwise, the entry in `rolledup` represent rolled-up information for a
        //   (jobid,ppid,command) triple
        //
        // - There is a hash table `index` that maps the (jobid,ppid,command) triple to the entry in
        //   `rolledup`, if any
        //
        // - When we're done rolling up, we print the `rolledup` table.
        //
        // Filtering is performed after rolling up, so if a rolled-up job has a bunch of dinky
        // processes that together push it over the filtering limit then it will be printed.  This
        // is probably the right thing.

        let mut rolledup = vec![];
        let mut index = HashMap::<(JobID, Pid, &str), usize>::new();
        for proc_info in proc_by_pid.values() {
            if proc_info.job_id == 0 || proc_info.has_children {
                rolledup.push(proc_info.clone());
            } else {
                let key = (proc_info.job_id, proc_info.ppid, proc_info.command);
                if let Some(x) = index.get(&key) {
                    let p = &mut rolledup[*x];
                    p.cpu_percentage += proc_info.cpu_percentage;
                    p.cputime_sec += proc_info.cputime_sec;
                    p.mem_percentage += proc_info.mem_percentage;
                    p.mem_size_kib += proc_info.mem_size_kib;
                    p.rssanon_kib += proc_info.rssanon_kib;
                    union_gpuset(&mut p.gpu_cards, &proc_info.gpu_cards);
                    p.gpu_percentage += proc_info.gpu_percentage;
                    p.gpu_mem_percentage += proc_info.gpu_mem_percentage;
                    p.gpu_mem_size_kib += proc_info.gpu_mem_size_kib;
                    p.rolledup += 1;
                } else {
                    let x = rolledup.len();
                    index.insert(key, x);
                    rolledup.push(proc_info.clone());
                    // We do not increment the clone's `rolledup` counter here because that counter
                    // counts how many *other* records have been rolled into the canonical one, 0
                    // means "no interesting information" and need not be printed.
                }
            }
        }
        rolledup
    } else {
        proc_by_pid
            .drain()
            .map(|(_, v)| v)
            .collect::<Vec<ProcInfo>>()
    };

    let must_print = opts.always_print_something;
    let candidates = candidates
        .drain(0..)
        .filter(|proc_info| filter_proc(proc_info, &print_params))
        .collect::<Vec<ProcInfo>>();

    let mut did_print = false;
    for c in candidates {
        match print_record(
            &mut writer,
            &print_params,
            &c,
            if !did_print {
                Some(&per_cpu_secs)
            } else {
                None
            },
            if !did_print { Some(&gpu_info) } else { None },
        ) {
            Ok(did_print_one) => did_print = did_print_one || did_print,
            Err(_) => {
                // Discard the error: there's nothing very sensible we can do at this point if the
                // write failed, and it will fail if we cut off a pipe, for example, see #132.  I
                // guess one can argue whether we should try the next record, but it seems sensible
                // to just bail out and hope for the best.
                break;
            }
        }
    }

    if !did_print && must_print {
        // Print a synthetic record
        let synth = ProcInfo {
            user: "_sonar_",
            _uid: 0,
            command: "_heartbeat_",
            pid: 0,
            ppid: 0,
            rolledup: 0,
            is_system_job: true,
            has_children: false,
            job_id: 0,
            cpu_percentage: 0.0,
            cputime_sec: 0,
            mem_percentage: 0.0,
            mem_size_kib: 0,
            rssanon_kib: 0,
            gpu_cards: empty_gpuset(),
            gpu_percentage: 0.0,
            gpu_mem_percentage: 0.0,
            gpu_mem_size_kib: 0,
            gpu_status: GpuStatus::Ok,
        };
        // Discard the error, see above.
        let _ = print_record(
            &mut writer,
            &print_params,
            &synth,
            if !did_print {
                Some(&per_cpu_secs)
            } else {
                None
            },
            if !did_print { Some(&gpu_info) } else { None },
        );
    }

    // Discard the error code, see above.
    let _ = writer.flush();
}

fn add_key(
    mut s: String,
    key: &str,
    cards: &[gpu::CardState],
    extract: fn(&gpu::CardState) -> String,
) -> String {
    let mut vs = "".to_string();
    let mut any = false;
    let mut first = true;
    for c in cards {
        let v = extract(c);
        if !first {
            vs += "|";
        }
        if !v.is_empty() {
            any = true;
            vs = vs + &v;
        }
        first = false;
    }
    if any {
        if !s.is_empty() {
            s += ",";
        }
        s + key + "=" + &vs
    } else {
        s
    }
}

fn nonzero(x: i64) -> String {
    if x == 0 {
        "".to_string()
    } else {
        format!("{:?}", x)
    }
}

fn filter_proc(proc_info: &ProcInfo, params: &PrintParameters) -> bool {
    let mut included = false;

    // The logic here is that if any of the inclusion filters are provided, then the set of those
    // that are provided constitute the entire inclusion filter, and the record must pass at least
    // one of those to be included.  Otherwise, when none of the filters are provided then the
    // record is included by default.

    if params.opts.min_cpu_percent.is_some()
        || params.opts.min_mem_percent.is_some()
        || params.opts.min_cpu_time.is_some()
    {
        if let Some(cpu_cutoff_percent) = params.opts.min_cpu_percent {
            if proc_info.cpu_percentage >= cpu_cutoff_percent {
                included = true;
            }
        }
        if let Some(mem_cutoff_percent) = params.opts.min_mem_percent {
            if proc_info.mem_percentage >= mem_cutoff_percent {
                included = true;
            }
        }
        if let Some(cpu_cutoff_time) = params.opts.min_cpu_time {
            if proc_info.cputime_sec >= cpu_cutoff_time {
                included = true;
            }
        }
    } else {
        included = true;
    }

    // The exclusion filters apply after the inclusion filters and the record must pass all of the
    // ones that are provided.

    if params.opts.exclude_system_jobs && proc_info.is_system_job {
        included = false;
    }
    if !params.opts.exclude_users.is_empty()
        && params
            .opts
            .exclude_users
            .iter()
            .any(|x| *x == proc_info.user)
    {
        included = false;
    }
    if !params.opts.exclude_commands.is_empty()
        && params
            .opts
            .exclude_commands
            .iter()
            .any(|x| proc_info.command.starts_with(x))
    {
        included = false;
    }

    included
}

struct PrintParameters<'a> {
    hostname: &'a str,
    timestamp: &'a str,
    version: &'a str,
    opts: &'a PsOptions<'a>,
}

fn print_record(
    writer: &mut dyn io::Write,
    params: &PrintParameters,
    proc_info: &ProcInfo,
    per_cpu_secs: Option<&[u64]>,
    gpu_info: Option<&str>,
) -> Result<bool> {
    // Mandatory fields.

    let mut fields = vec![
        format!("v={}", params.version),
        format!("time={}", params.timestamp),
        format!("host={}", params.hostname),
        format!("user={}", proc_info.user),
        format!("cmd={}", proc_info.command),
    ];

    // Only print optional fields whose values are not their defaults.  The defaults are defined in
    // README.md.  The values there must agree with those used by Jobanalyzer's parser.

    if proc_info.job_id != 0 {
        fields.push(format!("job={}", proc_info.job_id));
    }
    if proc_info.rolledup == 0 && proc_info.pid != 0 {
        // pid must be 0 for rolledup > 0 as there is no guarantee that there is any fixed
        // representative pid for a rolled-up set of processes: the set can change from run to run,
        // and sonar has no history.
        fields.push(format!("pid={}", proc_info.pid));
    }
    if proc_info.ppid != 0 {
        fields.push(format!("ppid={}", proc_info.ppid));
    }
    if proc_info.cpu_percentage != 0.0 {
        fields.push(format!("cpu%={}", three_places(proc_info.cpu_percentage)));
    }
    if proc_info.mem_size_kib != 0 {
        fields.push(format!("cpukib={}", proc_info.mem_size_kib));
    }
    if proc_info.rssanon_kib != 0 {
        fields.push(format!("rssanonkib={}", proc_info.rssanon_kib));
    }
    if let Some(ref cards) = proc_info.gpu_cards {
        if cards.is_empty() {
            // Nothing
        } else {
            fields.push(format!(
                "gpus={}",
                cards
                    .iter()
                    .map(|&num| num.to_string())
                    .collect::<Vec<String>>()
                    .join(",")
            ))
        }
    } else {
        fields.push("gpus=unknown".to_string());
    }
    if proc_info.gpu_percentage != 0.0 {
        fields.push(format!("gpu%={}", three_places(proc_info.gpu_percentage)));
    }
    if proc_info.gpu_mem_percentage != 0.0 {
        fields.push(format!(
            "gpumem%={}",
            three_places(proc_info.gpu_mem_percentage)
        ));
    }
    if proc_info.gpu_mem_size_kib != 0 {
        fields.push(format!("gpukib={}", proc_info.gpu_mem_size_kib));
    }
    if proc_info.cputime_sec != 0 {
        fields.push(format!("cputime_sec={}", proc_info.cputime_sec));
    }
    if proc_info.gpu_status != GpuStatus::Ok {
        fields.push(format!("gpufail={}", proc_info.gpu_status as i32));
    }
    if proc_info.rolledup > 0 {
        fields.push(format!("rolledup={}", proc_info.rolledup));
    }
    if params.opts.load {
        if let Some(cpu_secs) = per_cpu_secs {
            if !cpu_secs.is_empty() {
                fields.push(format!("load={}", encode_cpu_secs_base45el(cpu_secs)));
            }
        }
        if let Some(gpu_info) = gpu_info {
            if !gpu_info.is_empty() {
                fields.push(format!("gpuinfo={gpu_info}"));
            }
        }
    }

    let mut s = "".to_string();
    for f in fields {
        if !s.is_empty() {
            s += ","
        }
        s += &csv_quote(&f);
    }
    s += "\n";

    let _ = writer.write(s.as_bytes())?;

    Ok(true)
}

// Encode a nonempty u64 array compactly.
//
// The output must be ASCII text (32 <= c < 128), ideally without ',' or '"' or '\' or ' ' to not
// make it difficult for the various output formats we use.  Also avoid DEL, because it is a weird
// control character.
//
// We have many encodings to choose from, see https://github.com/NordicHPC/sonar/issues/178.
//
// The values to be represented are always cpu seconds of active time since boot, one item per cpu,
// and it is assumed that they are roughly in the vicinity of each other (the largest is rarely more
// than 4x the smallest, say).  The assumption does not affect correctness, only compactness.
//
// The encoding first finds the minimum input value and subtracts that from all entries.  The
// minimum value, and all the entries, are then emitted as unsigned little-endian base-45 with the
// initial digit chosen from a different character set to indicate that it is initial.

fn encode_cpu_secs_base45el(cpu_secs: &[u64]) -> String {
    let base = *cpu_secs
        .iter()
        .reduce(std::cmp::min)
        .expect("Must have a non-empty array");
    let mut s = encode_u64_base45el(base);
    for x in cpu_secs {
        s += encode_u64_base45el(*x - base).as_str();
    }
    s
}

// The only character unused by the encoding, other than the ones we're not allowed to use, is '='.
const BASE: u64 = 45;
const INITIAL: &[u8] = "(){}[]<>+-abcdefghijklmnopqrstuvwxyz!@#$%^&*_".as_bytes();
const SUBSEQUENT: &[u8] = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ~|';:.?/`".as_bytes();

fn encode_u64_base45el(mut x: u64) -> String {
    let mut s = String::from(INITIAL[(x % BASE) as usize] as char);
    x /= BASE;
    while x > 0 {
        s.push(SUBSEQUENT[(x % BASE) as usize] as char);
        x /= BASE;
    }
    s
}

#[test]
pub fn test_encoding() {
    assert!(INITIAL.len() == BASE as usize);
    assert!(SUBSEQUENT.len() == BASE as usize);
    // This should be *1, *0, *29, *43, 1, *11 with * denoting an INITIAL char.
    let v = vec![1, 30, 89, 12];
    println!("{}", encode_cpu_secs_base45el(&v));
    assert!(encode_cpu_secs_base45el(&v) == ")(t*1b");
}
