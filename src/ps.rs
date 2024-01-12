#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

extern crate log;
extern crate num_cpus;

use crate::amd;
use crate::jobs;
use crate::nvidia;
use crate::process;
use crate::procfs;
use crate::util::three_places;
use crate::procfsapi;

use csv::{Writer, WriterBuilder};
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time;

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
        lhs.as_mut().unwrap().extend(rhs.as_ref().unwrap());
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
    rolledup: usize,
    is_system_job: bool,
    job_id: usize,
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_percentage: f64,
    mem_size_kib: usize,
    resident_kib: usize,
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
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_percentage: f64,
    mem_size_kib: usize,
    resident_kib: usize,
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
            e.resident_kib += resident_kib;
            union_gpuset(&mut e.gpu_cards, gpu_cards);
            e.gpu_percentage += gpu_percentage;
            e.gpu_mem_percentage += gpu_mem_percentage;
            e.gpu_mem_size_kib += gpu_mem_size_kib;
        })
        .or_insert(ProcInfo {
            user,
            _uid: uid,
            command,
            pid,
            rolledup: 0,
            is_system_job: uid < 1000,
            job_id: lookup_job_by_pid(pid),
            cpu_percentage,
            cputime_sec,
            mem_percentage,
            mem_size_kib,
            resident_kib,
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
}

pub fn create_snapshot(jobs: &mut dyn jobs::JobManager, opts: &PsOptions, timestamp: &str) {
    // If a lock file was requested, create one before the operation, exit early if it already
    // exists, and if we performed the operation, remove the file afterwards.  Otherwise, just
    // perform the operation.
    //
    // However if a signal arrives in the middle of the operation and terminates the program the
    // lock file may be left on disk.  Assuming no bugs, the interesting signals are SIGHUP,
    // SIGTERM, SIGINT, and SIGQUIT.  Of these, only SIGHUP and SIGTERM are really interesting
    // because they are sent by the OS or by job control (and will often be followed by SIGKILL if
    // not honored within some reasonable time); INT/QUIT are sent by a user in response to keyboard
    // action and more typical during development/debugging.
    //
    // So THE MAIN REASON TO HANDLE SIGNALS is to make sure we're not killed while we hold the lock
    // file, during normal operation.  To do that, we just need to ignore the signal.
    //
    // But if a handled signal is received, it would be sensible to exit as quickly as possible and
    // with minimal risk of hanging.  And to do that, we record the signal and then check the flag
    // often, and we avoid starting things if the flag is set, and we produce no output (if
    // possible) once the flag becomes set, yet produce complete output if any output at all.
    //
    // There's no reason to limit the signal handler to the case when we have a lock file, the same
    // logic can apply to both paths.

    let interrupted = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&interrupted)).unwrap();
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&interrupted)).unwrap();

    if let Some(ref dirname) = opts.lockdir {
        let mut created = false;
        let mut failed = false;
        let mut skip = false;
        let hostname = hostname::get().unwrap().into_string().unwrap();

        let mut p = PathBuf::new();
        p.push(dirname);
        p.push("sonar-lock.".to_string() + &hostname);

        if interrupted.load(Ordering::Relaxed) {
            return
        }

        // create_new() requests atomic creation, if the file exists we'll error out.
        match std::fs::File::options().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                created = true;
                let pid = std::process::id();
                match f.write(format!("{}", pid).as_bytes()) {
                    Ok(_) => {}
                    Err(_) => { failed = true; }
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
            do_create_snapshot(jobs, opts, timestamp, &interrupted);

            // Testing code: If we got the lockfile and produced a report, wait 10s after producing
            // it while holding onto the lockfile.  It is then possible to run sonar in that window
            // while the lockfile is being held, to ensure the second process exits immediately.
            match std::env::var("SONARTEST_WAIT_LOCKFILE") {
                Ok(_) => { thread::sleep(time::Duration::new(10, 0)); }
                Err(_) => {}
            }
        }

        if created {
            match std::fs::remove_file(p) {
                Ok(_) => {}
                Err(_) => { failed = true; }
            }
        }

        if skip {
            log::info!("Lockfile present, exiting");
        }
        if failed {
            log::error!("Unable to properly manage or delete lockfile");
        }
    } else {
        do_create_snapshot(jobs, opts, timestamp, &interrupted);
    }
}

fn do_create_snapshot(jobs: &mut dyn jobs::JobManager, opts: &PsOptions, timestamp: &str, interrupted: &Arc<AtomicBool>) {
    let no_gpus = empty_gpuset();
    let mut proc_by_pid = ProcTable::new();

    /* Useful debugging code for ps vs procfs
    {
        use std::fs::File;
        use io::Write;

        let mut ps1 = procfs::get_process_information().expect("procfs");
        ps1.sort_by_key(|a| a.pid);

        let mut ps2 = process::get_process_information().expect("ps");
        ps2.sort_by_key(|b| b.pid);

        let mut ps1file = File::create("procfs-out.txt").unwrap();
        for process::Process { pid, uid, user, cpu_pct, mem_pct, cputime_sec, mem_size_kib, command, ppid, session }  in ps1 {
            writeln!(&mut ps1file, "pid {pid} ppid {ppid} uid {uid} user {user} cpu_pct {cpu_pct} mem_pct {mem_pct} mem_size_kib {mem_size_kib} cputime_sec {cputime_sec} session {session} comm {command}").unwrap();
        }

        let mut ps2file = std::fs::File::create("ps-out.txt").unwrap();
        for process::Process { pid, uid, user, cpu_pct, mem_pct, cputime_sec, mem_size_kib, command, ppid, session }  in ps2 {
            writeln!(&mut ps2file, "pid {pid} ppid {ppid} uid {uid} user {user} cpu_pct {cpu_pct} mem_pct {mem_pct} mem_size_kib {mem_size_kib} cputime_sec {cputime_sec} session {session} comm {command}").unwrap();
        }
    }
    */

    if interrupted.load(Ordering::Relaxed) {
        return
    }

    let fs = procfsapi::RealFS::new();

    // The total RAM installed is in the `MemTotal` field of /proc/meminfo.  We need this for
    // various things.  Not getting it is a hard error.

    let memtotal_kib =
        match procfs::get_memtotal(&fs) {
            Ok(n) => n,
            Err(e) => {
                log::error!("Could not get installed memory: {}", e);
                return;
            }
        };

    let procinfo_probe = {
        match procfs::get_process_information(&fs, memtotal_kib) {
            Ok(result) => Ok(result),
            Err(msg) => {
                eprintln!("INFO: procfs failed: {}", msg);
                process::get_process_information()
            }
        }
    };
    if let Err(e) = procinfo_probe {
        // This is a hard error, we need this information for everything.
        log::error!("CPU process listing failed: {:?}", e);
        return;
    }
    let procinfo_output = &procinfo_probe.unwrap();

    // The table of users is needed to get GPU information, see comments at UserTable.
    let mut user_by_pid = UserTable::new();
    for proc in procinfo_output {
        user_by_pid.insert(proc.pid, (&proc.user, proc.uid));
    }

    let mut lookup_job_by_pid = |pid: Pid| jobs.job_id_from_pid(pid, procinfo_output);

    for proc in procinfo_output {
        add_proc_info(
            &mut proc_by_pid,
            &mut lookup_job_by_pid,
            &proc.user,
            proc.uid,
            &proc.command,
            proc.pid,
            proc.cpu_pct,
            proc.cputime_sec,
            proc.mem_pct,
            proc.mem_size_kib,
            proc.resident_kib,
            &no_gpus, // gpu_cards
            0.0,      // gpu_percentage
            0.0,      // gpu_mem_percentage
            0,
        ); // gpu_mem_size_kib
    }

    if interrupted.load(Ordering::Relaxed) {
        return
    }

    // When a GPU fails it may be a transient error or a permanent error, but either
    // way sonar does not know.  We just record the failure.

    let mut gpu_status = GpuStatus::Ok;

    let nvidia_probe = nvidia::get_nvidia_information(&user_by_pid);
    match nvidia_probe {
        Err(_e) => {
            gpu_status = GpuStatus::UnknownFailure;
            // This is a soft failure, surfaced through dashboards; we do not want mail about it
            // under normal circumstances.
            //log::error!("GPU (Nvidia) process listing failed: {:?}", e);
        }
        Ok(ref nvidia_output) => {
            for proc in nvidia_output {
                add_proc_info(
                    &mut proc_by_pid,
                    &mut lookup_job_by_pid,
                    &proc.user,
                    proc.uid,
                    &proc.command,
                    proc.pid,
                    0.0, // cpu_percentage
                    0,   // cputime_sec
                    0.0, // mem_percentage
                    0,   // mem_size_kib
                    0,   // resident_kib
                    &singleton_gpuset(proc.device),
                    proc.gpu_pct,
                    proc.mem_pct,
                    proc.mem_size_kib,
                );
            }
        }
    }

    if interrupted.load(Ordering::Relaxed) {
        return
    }

    let amd_probe = amd::get_amd_information(&user_by_pid);
    match amd_probe {
        Err(_e) => {
            gpu_status = GpuStatus::UnknownFailure;
            // This is a soft failure, surfaced through dashboards; we do not want mail about it
            // under normal circumstances.
            //log::error!("GPU (AMD) process listing failed: {:?}", e);
        }
        Ok(ref amd_output) => {
            for proc in amd_output {
                add_proc_info(
                    &mut proc_by_pid,
                    &mut lookup_job_by_pid,
                    &proc.user,
                    proc.uid,
                    &proc.command,
                    proc.pid,
                    0.0, // cpu_percentage
                    0,   // cputime_sec
                    0.0, // mem_percentage
                    0,   // mem_size_kib
                    0,   // resident_kib
                    &singleton_gpuset(proc.device),
                    proc.gpu_pct,
                    proc.mem_pct,
                    proc.mem_size_kib,
                );
            }
        }
    }

    // If there was a gpu failure, signal it in all the process structures.  This is pretty
    // conservative and increases data volume, but it means that the information is not lost so long
    // as not all records from this sonar run are filtered out by the front end.

    if gpu_status != GpuStatus::Ok {
        for proc_info in proc_by_pid.values_mut() {
            proc_info.gpu_status = gpu_status;
        }
    }

    if interrupted.load(Ordering::Relaxed) {
        return
    }

    // Once we start printing we'll print everything and not check the interrupted flag any more.

    let mut writer = WriterBuilder::new()
        .flexible(true)
        .from_writer(io::stdout());

    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let print_params = PrintParameters {
        hostname: &hostname,
        timestamp: timestamp,
        num_cores,
        memtotal_kib,
        version: VERSION,
        opts,
    };

    let mut did_print = false;
    let must_print = opts.always_print_something;
    if opts.rollup {
        // This is a little complicated because jobs with job_id 0 cannot be rolled up.
        //
        // - There is an array `rolledup` of ProcInfo nodes that represent rolled-up data
        //
        // - When the job ID of a job in `proc_by_pid` is zero, the entry in `rolledup` is a copy of
        //   that job; these jobs cannot be rolled up (this is why it's complicated)
        //
        // - Otherwise, the entry in `rolledup` represent rolled-up information for a (job, command)
        //   pair
        //
        // - There is a hash table `index` that maps the (job, command) pair to the entry in
        //   `rolledup`, if any
        //
        // - When we're done rolling up, we print the `rolledup` table.
        //
        // Filtering is performed after rolling up, so if a rolled-up job has a bunch of dinky
        // processes that together push it over the filtering limit then it will be printed.  This
        // is probably the right thing.

        let mut rolledup = vec![];
        let mut index = HashMap::<(JobID, &str), usize>::new();
        for proc_info in proc_by_pid.values() {
            if proc_info.job_id == 0 {
                rolledup.push(proc_info.clone());
            } else {
                let key = (proc_info.job_id, proc_info.command);
                if let Some(x) = index.get(&key) {
                    let p = &mut rolledup[*x];
                    p.cpu_percentage += proc_info.cpu_percentage;
                    p.cputime_sec += proc_info.cputime_sec;
                    p.mem_percentage += proc_info.mem_percentage;
                    p.mem_size_kib += proc_info.mem_size_kib;
                    p.resident_kib += proc_info.resident_kib;
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
        for r in rolledup {
            did_print = print_record(&mut writer, &print_params, &r, false) || did_print;
        }
    } else {
        for (_, proc_info) in proc_by_pid {
            did_print = print_record(&mut writer, &print_params, &proc_info, false) || did_print;
        }
    }

    if !did_print && must_print {
        // Print a synthetic record
        let synth = ProcInfo {
            user: "_sonar_",
            _uid: 0,
            command: "_heartbeat_",
            pid: 0,
            rolledup: 0,
            is_system_job: true,
            job_id: 0,
            cpu_percentage: 0.0,
            cputime_sec: 0,
            mem_percentage: 0.0,
            mem_size_kib: 0,
            resident_kib: 0,
            gpu_cards: empty_gpuset(),
            gpu_percentage: 0.0,
            gpu_mem_percentage: 0.0,
            gpu_mem_size_kib: 0,
            gpu_status: GpuStatus::Ok,
        };
        print_record(&mut writer, &print_params, &synth, true);
    }

    writer.flush().unwrap();
}

struct PrintParameters<'a> {
    hostname: &'a str,
    timestamp: &'a str,
    num_cores: usize,
    memtotal_kib: usize,
    version: &'a str,
    opts: &'a PsOptions<'a>,
}

fn print_record<W: io::Write>(
    writer: &mut Writer<W>,
    params: &PrintParameters,
    proc_info: &ProcInfo,
    must_print: bool,
) -> bool {
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

    if !included && !must_print {
        return false;
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

    if !included && !must_print {
        return false;
    }

    let gpus_comma_separated = if let Some(ref cards) = proc_info.gpu_cards {
        if cards.is_empty() {
            "none".to_string()
        } else {
            cards
                .iter()
                .map(|&num| num.to_string())
                .collect::<Vec<String>>()
                .join(",")
        }
    } else {
        "unknown".to_string()
    };

    let mut fields = vec![
        format!("v={}", params.version),
        format!("time={}", params.timestamp),
        format!("host={}", params.hostname),
        format!("cores={}", params.num_cores),
        format!("memtotalkib={}", params.memtotal_kib),
        format!("user={}", proc_info.user),
        format!("job={}", proc_info.job_id),
        format!(
            "pid={}",
            if proc_info.rolledup > 0 {
                0
            } else {
                proc_info.pid
            }
        ),
        format!("cmd={}", proc_info.command),
        format!("cpu%={}", three_places(proc_info.cpu_percentage)),
        format!("cpukib={}", proc_info.mem_size_kib),
        format!("rsskib={}", proc_info.resident_kib),
        format!("gpus={gpus_comma_separated}"),
        format!("gpu%={}", three_places(proc_info.gpu_percentage)),
        format!("gpumem%={}", three_places(proc_info.gpu_mem_percentage)),
        format!("gpukib={}", proc_info.gpu_mem_size_kib),
        format!("cputime_sec={}", proc_info.cputime_sec),
    ];
    if proc_info.gpu_status != GpuStatus::Ok {
        fields.push(format!("gpufail={}", proc_info.gpu_status as i32));
    }
    if proc_info.rolledup > 0 {
        fields.push(format!("rolledup={}", proc_info.rolledup));
    }
    writer.write_record(fields).unwrap();

    true
}
