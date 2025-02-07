#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::gpuapi;
use crate::gpuset;
use crate::log;
#[cfg(test)]
use crate::mockjobs;
#[cfg(test)]
use crate::mocksystem;
use crate::output;
use crate::procfs;
use crate::systemapi;
use crate::util::three_places;

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

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
    gpu_cards: gpuset::GpuSet,
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
    gpu_cards: &gpuset::GpuSet,
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
            gpuset::union_gpuset(&mut e.gpu_cards, gpu_cards);
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

#[derive(Default)]
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
    pub json: bool,
}

pub fn create_snapshot(
    writer: &mut dyn io::Write,
    system: &dyn systemapi::SystemAPI,
    opts: &PsOptions,
) {
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

    system.handle_interruptions();

    if let Some(ref dirname) = opts.lockdir {
        let mut created = false;
        let mut failed = false;
        let mut skip = false;
        let hostname = system.get_hostname();

        let mut p = PathBuf::new();
        p.push(dirname);
        p.push("sonar-lock.".to_string() + &hostname);

        if system.is_interrupted() {
            return;
        }

        // create_new() requests atomic creation, if the file exists we'll error out.
        match system.create_lock_file(&p) {
            Ok(mut f) => {
                created = true;
                let pid = system.get_pid();
                match f.write(format!("{pid}").as_bytes()) {
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
            do_create_snapshot(writer, system, opts);

            // Testing code: If we got the lockfile and produced a report, wait 10s after producing
            // it while holding onto the lockfile.  It is then possible to run sonar in that window
            // while the lockfile is being held, to ensure the second process exits immediately.
            #[cfg(debug_assertions)]
            if std::env::var("SONARTEST_WAIT_LOCKFILE").is_ok() {
                std::thread::sleep(std::time::Duration::new(10, 0));
            }
        }

        if created {
            match system.remove_lock_file(p) {
                Ok(_) => {}
                Err(_) => {
                    failed = true;
                }
            }
        }

        // These log/error messages can't sensibly be piggybacked on the normal output, since the
        // output has been sent - it can't be delayed until this point, as the lockfile is meant to
        // ensure that if bugs in the printing code make the program hang forever then no other
        // sonar process is started.
        //
        // If the error persists then no messages will arrive at the target and an alert that
        // triggers on the absence of traffic should alert somebody to the problem.

        if skip {
            // Test cases depend on this exact message.
            log::info("Lockfile present, exiting");
        }
        if failed {
            log::error("Unable to properly manage or delete lockfile");
        }
    } else {
        do_create_snapshot(writer, system, opts);
    }
}

fn do_create_snapshot(
    writer: &mut dyn io::Write,
    system: &dyn systemapi::SystemAPI,
    opts: &PsOptions,
) {
    let hostname = system.get_hostname();
    let print_params = PrintParameters {
        hostname: &hostname,
        timestamp: &system.get_timestamp(),
        version: &system.get_version(),
        flat_data: !opts.json,
        opts,
    };

    match collect_data(system, &print_params) {
        output::Value::A(elts) => {
            for i in 0..elts.len() {
                output::write_csv(writer, elts.at(i));
            }
        }
        obj @ output::Value::O(_) => {
            output::write_json(writer, &obj);
        }
        output::Value::E() => {
            // interrupted, don't print anything
        }
        _ => {
            panic!("Should not happen")
        }
    }
}

// If this returns an output::Value::O then that is an object to write (eg JSON), otherwise it must
// be an output::Value::A and each should be written individually (eg CSV), or it is
// output::Value::E, in which case we were interrupted.  The first two cases are controlled by
// print_params.flat_data.

fn collect_data(
    system: &dyn systemapi::SystemAPI,
    print_params: &PrintParameters,
) -> output::Value {
    match do_collect_data(system, print_params) {
        Ok(output::Value::A(mut elts)) => {
            if elts.len() == 0 && print_params.opts.always_print_something {
                elts.push_o(make_heartbeat(&print_params))
            }
            output::Value::A(elts)
        }
        Ok(obj @ output::Value::O(_)) => obj,
        Ok(empty @ output::Value::E()) => empty,
        Ok(_) => {
            panic!("Should not happen")
        }
        Err(error) => {
            let mut hb = make_heartbeat(&print_params);
            hb.push_s("error", error);
            if print_params.flat_data {
                output::Value::A(output::Array::from_vec(vec![output::Value::O(hb)]))
            } else {
                output::Value::O(hb)
            }
        }
    }
}

fn make_heartbeat(print_params: &PrintParameters) -> output::Object {
    let mut fields = output::Object::new();
    fields.push_s("v", print_params.version.to_string());
    fields.push_s("time", print_params.timestamp.to_string());
    fields.push_s("host", print_params.hostname.to_string());
    fields.push_s("user", "_sonar_".to_string());
    fields.push_s("cmd", "_heartbeat_".to_string());
    fields
}

fn do_collect_data<'a>(
    system: &dyn systemapi::SystemAPI,
    print_params: &PrintParameters,
) -> Result<output::Value, String> {
    let fs = system.get_procfs();
    let gpus = system.get_gpus();

    let no_gpus = gpuset::empty_gpuset();
    let mut proc_by_pid = ProcTable::new();

    if system.is_interrupted() {
        return Ok(output::Value::E());
    }

    // The total RAM installed is in the `MemTotal` field of /proc/meminfo.  We need this for
    // various things.  Not getting it is a hard error.

    let memtotal_kib = procfs::get_memtotal_kib(fs)?;
    let (procinfo_output, _cpu_total_secs, per_cpu_secs) =
        procfs::get_process_information(system, memtotal_kib)?;

    let pprocinfo_output = &procinfo_output;

    // The table of users is needed to get GPU information, see comments at UserTable.
    let mut user_by_pid = UserTable::new();
    for proc in pprocinfo_output.values() {
        user_by_pid.insert(proc.pid, (&proc.user, proc.uid));
    }

    let mut lookup_job_by_pid = |pid: Pid| system.get_jobs().job_id_from_pid(pid, pprocinfo_output);

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

    if system.is_interrupted() {
        return Ok(output::Value::E());
    }

    // When a GPU fails it may be a transient error or a permanent error, but either way sonar does
    // not know.  We just record the failure.
    //
    // This is a soft failure, surfaced through dashboards; we do not want mail about it under
    // normal circumstances.
    let mut gpu_status = GpuStatus::Ok;

    let gpu_utilization: Vec<gpuapi::Process>;
    let mut gpu_info: Option<output::Object> = None;
    match gpus.probe() {
        None => {}
        Some(mut gpu) => {
            match gpu.get_card_utilization() {
                Err(_) => {
                    gpu_status = GpuStatus::UnknownFailure;
                }
                Ok(ref cards) => {
                    let mut s = output::Object::new();
                    s = add_key(s, "fan%", cards, |c: &gpuapi::CardState| {
                        nonzero(c.fan_speed_pct as i64)
                    });
                    s = add_key(s, "mode", cards, |c: &gpuapi::CardState| {
                        if c.compute_mode == "Default" {
                            output::Value::E()
                        } else {
                            output::Value::S(c.compute_mode.clone())
                        }
                    });
                    s = add_key(s, "perf", cards, |c: &gpuapi::CardState| {
                        output::Value::S(c.perf_state.clone())
                    });
                    // Reserved memory is really not interesting, it's possible it would have been
                    // interesting as part of the card configuration.
                    //s = add_key(s, "mreskib", cards, |c: &gpuapi::CardState| nonzero(c.mem_reserved_kib));
                    s = add_key(s, "musekib", cards, |c: &gpuapi::CardState| {
                        nonzero(c.mem_used_kib)
                    });
                    s = add_key(s, "cutil%", cards, |c: &gpuapi::CardState| {
                        nonzero(c.gpu_utilization_pct as i64)
                    });
                    s = add_key(s, "mutil%", cards, |c: &gpuapi::CardState| {
                        nonzero(c.mem_utilization_pct as i64)
                    });
                    s = add_key(s, "tempc", cards, |c: &gpuapi::CardState| {
                        nonzero(c.temp_c.into())
                    });
                    s = add_key(s, "poww", cards, |c: &gpuapi::CardState| {
                        nonzero(c.power_watt.into())
                    });
                    s = add_key(s, "powlimw", cards, |c: &gpuapi::CardState| {
                        nonzero(c.power_limit_watt.into())
                    });
                    s = add_key(s, "cez", cards, |c: &gpuapi::CardState| {
                        nonzero(c.ce_clock_mhz.into())
                    });
                    s = add_key(s, "memz", cards, |c: &gpuapi::CardState| {
                        nonzero(c.mem_clock_mhz.into())
                    });
                    if !s.is_empty() {
                        gpu_info = Some(s);
                    }
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
                        // FIXME: This is not what we want, we can do better.
                        let command = match &proc.command {
                            Some(cmd) => cmd,
                            _ => "_unknown_",
                        };
                        add_proc_info(
                            &mut proc_by_pid,
                            &mut lookup_job_by_pid,
                            &proc.user,
                            proc.uid,
                            command,
                            proc.pid,
                            ppid,
                            has_children,
                            0.0, // cpu_percentage
                            0,   // cputime_sec
                            0.0, // mem_percentage
                            0,   // mem_size_kib
                            0,   // rssanon_kib
                            &proc.devices,
                            proc.gpu_pct,
                            proc.mem_pct,
                            proc.mem_size_kib,
                        );
                    }
                }
            }
        }
    }

    if system.is_interrupted() {
        return Ok(output::Value::E());
    }

    // If there was a gpu failure, signal it in all the process structures.  This is pretty
    // conservative and increases data volume, but it means that the information is not lost so long
    // as not all records from this sonar run are filtered out by the front end.

    if gpu_status != GpuStatus::Ok {
        for proc_info in proc_by_pid.values_mut() {
            proc_info.gpu_status = gpu_status;
        }
    }

    if system.is_interrupted() {
        return Ok(output::Value::E());
    }

    let mut candidates = if print_params.opts.rollup {
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
                    gpuset::union_gpuset(&mut p.gpu_cards, &proc_info.gpu_cards);
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

    let candidates = candidates
        .drain(0..)
        .filter(|proc_info| filter_proc(proc_info, print_params))
        .collect::<Vec<ProcInfo>>();

    let mut records: Vec<output::Object> = vec![];
    for c in candidates {
        records.push(generate_candidate(&c, print_params));
    }

    if print_params.flat_data {
        if print_params.opts.load && records.len() > 0 {
            if !per_cpu_secs.is_empty() {
                let mut a = output::Array::from_vec(
                    per_cpu_secs
                        .iter()
                        .map(|x| output::Value::U(*x))
                        .collect::<Vec<output::Value>>(),
                );
                a.set_encode_nonempty_base45();
                records[0].push_a("load", a);
            }
            if let Some(info) = gpu_info {
                records[0].push_o("gpuinfo", info);
            }
        }

        let mut result = output::Array::new();
        for v in records {
            result.push_o(v);
        }
        Ok(output::Value::A(result))
    } else {
        let mut datum = output::Object::new();
        datum.push_s("v", print_params.version.to_string());
        datum.push_s("time", print_params.timestamp.to_string());
        datum.push_s("host", print_params.hostname.to_string());
        if print_params.opts.load {
            if !per_cpu_secs.is_empty() {
                let a = output::Array::from_vec(
                    per_cpu_secs
                        .iter()
                        .map(|x| output::Value::U(*x))
                        .collect::<Vec<output::Value>>(),
                );
                datum.push_a("load", a);
            }
            if let Some(info) = gpu_info {
                datum.push_o("gpuinfo", info);
            }
        }
        let mut samples = output::Array::new();
        for o in records {
            samples.push_o(o);
        }
        datum.push_a("samples", samples);
        Ok(output::Value::O(datum))
    }
}

fn add_key<'a>(
    mut s: output::Object,
    key: &str,
    cards: &[gpuapi::CardState],
    extract: fn(&gpuapi::CardState) -> output::Value,
) -> output::Object {
    let mut vs = output::Array::new();
    let mut any_nonempty = false;
    vs.set_csv_separator("|".to_string());
    for c in cards {
        let v = extract(c);
        if let output::Value::E() = v {
        } else {
            any_nonempty = true;
        }
        vs.push(v);
    }
    if any_nonempty {
        s.push(key, output::Value::A(vs));
    }
    s
}

fn nonzero(x: i64) -> output::Value {
    if x == 0 {
        output::Value::E()
    } else {
        output::Value::I(x)
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
    flat_data: bool,
    opts: &'a PsOptions<'a>,
}

fn generate_candidate(proc_info: &ProcInfo, print_params: &PrintParameters) -> output::Object {
    let mut fields = output::Object::new();

    if print_params.flat_data {
        fields.push_s("v", print_params.version.to_string());
        fields.push_s("time", print_params.timestamp.to_string());
        fields.push_s("host", print_params.hostname.to_string());
    }

    fields.push_s("user", proc_info.user.to_string());
    fields.push_s("cmd", proc_info.command.to_string());

    // Only print optional fields whose values are not their defaults.  The defaults are defined in
    // README.md.  The values there must agree with those used by Jobanalyzer's parser.

    if proc_info.job_id != 0 {
        fields.push_u("job", proc_info.job_id as u64);
    }
    if proc_info.rolledup == 0 && proc_info.pid != 0 {
        // pid must be 0 for rolledup > 0 as there is no guarantee that there is any fixed
        // representative pid for a rolled-up set of processes: the set can change from run to run,
        // and sonar has no history.
        fields.push_u("pid", proc_info.pid as u64);
    }
    if proc_info.ppid != 0 {
        fields.push_u("ppid", proc_info.ppid as u64);
    }
    if proc_info.cpu_percentage != 0.0 {
        fields.push_f("cpu%", three_places(proc_info.cpu_percentage));
    }
    if proc_info.mem_size_kib != 0 {
        fields.push_u("cpukib", proc_info.mem_size_kib as u64);
    }
    if proc_info.rssanon_kib != 0 {
        fields.push_u("rssanonkib", proc_info.rssanon_kib as u64);
    }
    if let Some(ref cards) = proc_info.gpu_cards {
        if cards.is_empty() {
            // Nothing
        } else {
            fields.push_s(
                "gpus",
                cards
                    .iter()
                    .map(|&num| num.to_string())
                    .collect::<Vec<String>>()
                    .join(","),
            );
        }
    } else {
        fields.push_s("gpus", "unknown".to_string());
    }
    if proc_info.gpu_percentage != 0.0 {
        fields.push_f("gpu%", three_places(proc_info.gpu_percentage));
    }
    if proc_info.gpu_mem_percentage != 0.0 {
        fields.push_f("gpumem%", three_places(proc_info.gpu_mem_percentage));
    }
    if proc_info.gpu_mem_size_kib != 0 {
        fields.push_u("gpukib", proc_info.gpu_mem_size_kib as u64);
    }
    if proc_info.cputime_sec != 0 {
        fields.push_u("cputime_sec", proc_info.cputime_sec as u64);
    }
    if proc_info.gpu_status != GpuStatus::Ok {
        fields.push_u("gpufail", proc_info.gpu_status as u64);
    }
    if proc_info.rolledup > 0 {
        fields.push_u("rolledup", proc_info.rolledup as u64);
    }

    fields
}

#[test]
pub fn collect_data_test() {
    let opts = Default::default();
    let print_params = PrintParameters {
        hostname: "hello",
        timestamp: "2025-01-24T10:39:00+01:00",
        version: "0.99",
        flat_data: true,
        opts: &opts,
    };
    let system = mocksystem::MockSystem::new()
        .with_jobmanager(Box::new(mockjobs::MockJobManager {}))
        .freeze();
    match collect_data(&system, &print_params) {
        // flat_data, so should be array
        output::Value::A(a) => {
            // No data, so this should be length 1
            assert!(a.len() == 1);
            // Mock APIs, so we should have a heartbeat and an error
            match a.at(0) {
                output::Value::O(obj) => {
                    assert!(obj.get("error").is_some())
                }
                _ => {
                    assert!(false)
                }
            }
        }
        _ => {
            assert!(false);
        }
    }
}
