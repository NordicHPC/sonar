#![allow(clippy::len_zero)]

use crate::gpu;
use crate::json_tags::*;
use crate::output;
use crate::ps_newfmt::format_newfmt;
use crate::systemapi;
use crate::types::{JobID, Pid, Uid};

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
#[cfg(debug_assertions)]
use std::thread;
#[cfg(debug_assertions)]
use std::time;

#[derive(Default, Clone)]
pub enum Format {
    // There used to be CSV here.  We might add eg Protobuf.
    #[default]
    JSON,
}

#[derive(Default, Clone)]
pub struct PsOptions {
    pub rollup: bool,
    pub min_cpu_percent: Option<f64>,
    pub min_mem_percent: Option<f64>,
    pub min_cpu_time: Option<usize>,
    pub exclude_system_jobs: bool,
    pub exclude_users: Vec<String>,
    pub exclude_commands: Vec<String>,
    pub lockdir: Option<String>,
    pub token: String,
    pub load: bool,
    pub fmt: Format,
    pub cpu_util: bool,
}

#[cfg(feature = "daemon")]
pub struct State<'a> {
    system: &'a dyn systemapi::SystemAPI,
    opts: PsOptions,
}

#[cfg(feature = "daemon")]
impl<'a> State<'a> {
    pub fn new(system: &'a dyn systemapi::SystemAPI, opts: &PsOptions) -> State<'a> {
        State {
            system,
            opts: opts.clone(),
        }
    }

    pub fn run(&mut self) -> Vec<Vec<u8>> {
        let mut writer = Vec::new();
        do_create_snapshot(&mut writer, self.system, &self.opts);
        vec![writer]
    }
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
                thread::sleep(time::Duration::new(10, 0));
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
            log::warn!("Lockfile present, exiting");
        }
        if failed {
            log::error!("Unable to properly manage or delete lockfile");
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
    match collect_sample_data(system, opts) {
        Ok(Some(sample_data)) => match opts.fmt {
            Format::JSON => {
                let recoverable_errors = output::Array::new();
                let o = output::Value::O(format_newfmt(
                    &sample_data,
                    system,
                    opts,
                    recoverable_errors,
                ));
                output::write_json(writer, &o);
            }
        },
        Ok(None) => {
            // Interrupted, do not print anything
        }
        Err(error) => match opts.fmt {
            Format::JSON => {
                let mut envelope = output::newfmt_envelope(system, opts.token.clone(), &[]);
                envelope.push_a(
                    SAMPLE_ENVELOPE_ERRORS,
                    output::newfmt_one_error(system, error),
                );
                output::write_json(writer, &output::Value::O(envelope));
            }
        },
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// Data collection code

// The table mapping a Pid to user name / Uid is used by the GPU subsystems to provide information
// about users for the processes on the GPUS.

#[allow(dead_code)]
pub struct ProcessTable {
    by_pid: HashMap<Pid, (String, Uid)>,
}

impl ProcessTable {
    pub fn from_processes<T>(procs: &HashMap<T, systemapi::Process>) -> ProcessTable {
        let mut by_pid = HashMap::new();
        for proc in procs.values() {
            by_pid.insert(proc.pid, (proc.user.clone(), proc.uid));
        }
        ProcessTable { by_pid }
    }

    #[allow(dead_code)]
    pub fn lookup(&self, pid: Pid) -> (String, Uid) {
        match self.by_pid.get(&pid) {
            Some((name, uid)) => (name.to_string(), *uid),
            None => ("_user_unknown".to_string(), 1),
        }
    }
}

// ProcInfo holds per-process information gathered from multiple sources and tagged with a job ID.
// No processes are merged!  The job ID "0" means "unique job with no job ID".  That is, no consumer
// of this data, internal or external to the program, may treat separate processes with job ID "0"
// as part of the same job.

pub type ProcInfoTable = HashMap<Pid, ProcInfo>;

#[derive(Clone, Default)]
pub struct ProcInfo {
    pub user: String,
    pub command: String,
    pub pid: Pid,
    pub ppid: Pid,
    pub rolledup: usize,
    pub num_threads: usize,
    pub is_system_job: bool,
    pub has_children: bool,
    pub job_id: usize,
    pub is_slurm: bool,
    pub cpu_percentage: f64,
    pub cpu_util: f64,
    pub cputime_sec: usize,
    pub mem_percentage: f64,
    pub mem_size_kib: usize,
    pub data_read_kib: usize,
    pub data_written_kib: usize,
    pub data_cancelled_kib: usize,
    pub rssanon_kib: usize,
    pub gpus: GpuProcInfos,
    pub gpu_percentage: f64,
    pub gpu_mem_percentage: f64,
    pub gpu_mem_size_kib: usize,
    pub gpu_status: GpuStatus,
}

pub type GpuProcInfos = HashMap<gpu::Name, GpuProcInfo>;

#[derive(Clone, Default)]
pub struct GpuProcInfo {
    pub device: gpu::Name,
    pub gpu_util: u32,
    pub gpu_mem: u64,
    pub gpu_mem_util: u32,
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum GpuStatus {
    #[default]
    Ok = 0,
    UnknownFailure = 1,
}

pub struct SampleData {
    pub process_samples: Vec<ProcInfo>,
    pub gpu_samples: Option<Vec<gpu::CardState>>,
    pub cpu_samples: Vec<u64>,
    pub used_memory: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub runnable_entities: u64,
    pub existing_entities: u64,
}

fn collect_sample_data(
    system: &dyn systemapi::SystemAPI,
    opts: &PsOptions,
) -> Result<Option<SampleData>, String> {
    if system.is_interrupted() {
        return Ok(None);
    }

    let (_cpu_total_secs, per_cpu_secs) = system.compute_node_information()?;
    let memory = system.get_memory_in_kib()?;
    let (load1, load5, load15, runnable_entities, existing_entities) = system.compute_loadavg()?;
    let mut processes = system.compute_process_information()?;

    if opts.cpu_util {
        let utils = system.compute_cpu_utilization(&processes, 100)?;
        for (pid, cpu_util) in utils.iter() {
            processes.entry(*pid).and_modify(|e| {
                e.cpu_util = *cpu_util * 100.0;
            });
            // There is no or_insert case.  It may be that a process has gone away, and there's no
            // data for it, but not that a process has appeared during the utilization computation.
        }
    }

    let mut procinfo_by_pid = new_with_cpu_info(system, &processes);

    if system.is_interrupted() {
        return Ok(None);
    }

    let gpu_info: Option<Vec<gpu::CardState>>;
    (procinfo_by_pid, gpu_info) = add_gpu_info(procinfo_by_pid, system, &processes);

    if system.is_interrupted() {
        return Ok(None);
    }

    let mut candidates = if opts.rollup {
        rollup_processes(procinfo_by_pid)
    } else {
        procinfo_by_pid
            .drain()
            .map(|(_, v)| v)
            .collect::<Vec<ProcInfo>>()
    };

    let candidates = candidates
        .drain(0..)
        .filter(|proc_info| filter_proc(proc_info, opts))
        .collect::<Vec<ProcInfo>>();

    Ok(Some(SampleData {
        process_samples: candidates,
        gpu_samples: gpu_info,
        cpu_samples: per_cpu_secs,
        used_memory: memory.total - memory.available,
        load1,
        load5,
        load15,
        runnable_entities,
        existing_entities,
    }))
}

fn new_with_cpu_info(
    system: &dyn systemapi::SystemAPI,
    processes: &HashMap<Pid, systemapi::Process>,
) -> ProcInfoTable {
    let mut procinfo_by_pid = ProcInfoTable::new();
    for proc in processes.values() {
        let (job_id, is_slurm) = system
            .get_jobs()
            .job_id_from_pid(system, proc.pid, processes);
        procinfo_by_pid.insert(
            proc.pid,
            ProcInfo {
                user: proc.user.to_string(),
                command: proc.command.to_string(),
                pid: proc.pid,
                ppid: proc.ppid,
                is_system_job: proc.uid < 1000,
                has_children: proc.has_children,
                num_threads: if proc.num_threads > 0 {
                    proc.num_threads - 1
                } else {
                    0
                },
                job_id,
                is_slurm,
                cpu_percentage: proc.cpu_pct,
                cpu_util: proc.cpu_util,
                cputime_sec: proc.cputime_sec,
                mem_percentage: proc.mem_pct,
                mem_size_kib: proc.mem_size_kib,
                rssanon_kib: proc.rssanon_kib,
                data_read_kib: proc.data_read_kib,
                data_written_kib: proc.data_written_kib,
                data_cancelled_kib: proc.data_cancelled_kib,
                ..Default::default()
            },
        );
    }
    procinfo_by_pid
}

fn add_gpu_info(
    mut procinfo_by_pid: ProcInfoTable,
    system: &dyn systemapi::SystemAPI,
    processes: &HashMap<Pid, systemapi::Process>,
) -> (ProcInfoTable, Option<Vec<gpu::CardState>>) {
    // When a GPU fails it may be a transient error or a permanent error, but either way sonar does
    // not know.  We just record the failure.  This is a soft failure, surfaced through dashboards;
    // we do not want mail about it under normal circumstances.
    //
    // gpu_status is only used by the old format.  For the new format, every CardState carries
    // better error information.

    let mut gpu_status = GpuStatus::Ok;
    let mut gpu_info: Option<Vec<gpu::CardState>> = None;

    if let Some(gpu) = system.get_gpus().probe() {
        match gpu.get_card_utilization() {
            Ok(cards) => {
                gpu_info = Some(cards);
            }
            Err(_) => {
                gpu_status = GpuStatus::UnknownFailure;
            }
        }

        match gpu.get_process_utilization(&ProcessTable::from_processes(processes)) {
            Ok(mut gpu_utilization) => {
                // Tweak gpu_utilization: If any entry used more than 1 GPU we split the entry into
                // one per GPU, with data divided by the number of GPUs.
                let mut additional = vec![];
                for proc in &mut gpu_utilization {
                    let l = proc.devices.len();
                    if l > 1 {
                        let mut devices = vec![proc.devices[0].clone()];
                        std::mem::swap(&mut proc.devices, &mut devices);
                        proc.mem_size_kib /= l as u64;
                        proc.gpu_pct /= l as f32;
                        proc.mem_pct /= l as f32;
                        for d in devices.drain(1..) {
                            let mut c = proc.clone();
                            c.devices[0] = d;
                            additional.push(c)
                        }
                    }
                }
                gpu_utilization.extend(additional);

                for proc in &gpu_utilization {
                    assert!(proc.devices.len() == 1);
                    let (ppid, has_children) = processes
                        .get(&proc.pid)
                        .map_or((1, true), |p| (p.ppid, p.has_children));
                    // TODO: This is not what we want, we can do better.  (Should specify how.)
                    let command = match &proc.command {
                        Some(cmd) => cmd.clone(),
                        _ => "_unknown_".to_string(),
                    };
                    procinfo_by_pid
                        .entry(proc.pid)
                        .and_modify(|e| {
                            aggregate_gpu(
                                &mut e.gpus,
                                &proc.devices[0],
                                proc.gpu_pct as u32,
                                proc.mem_pct as u32,
                                proc.mem_size_kib,
                            );
                            e.gpu_percentage += proc.gpu_pct as f64;
                            e.gpu_mem_percentage += proc.mem_pct as f64;
                            e.gpu_mem_size_kib += proc.mem_size_kib as usize;
                        })
                        .or_insert_with(|| {
                            // This is a process the card knows about but that we did not see during
                            // the process scan.  It could be something that lingers in the card
                            // information, or a completely new process.  Either way, just make the
                            // best of it, if the process stays around we'll get good information
                            // later, otherwise it was dead anyway.
                            let (job_id, is_slurm) = system
                                .get_jobs()
                                .job_id_from_pid(system, proc.pid, processes);
                            ProcInfo {
                                user: proc.user.to_string(),
                                command,
                                pid: proc.pid,
                                ppid,
                                is_system_job: proc.uid < 1000,
                                num_threads: 0,
                                has_children,
                                job_id,
                                is_slurm,
                                gpus: singleton_gpu(
                                    &proc.devices[0],
                                    proc.gpu_pct as u32,
                                    proc.mem_pct as u32,
                                    proc.mem_size_kib,
                                ),
                                gpu_percentage: proc.gpu_pct as f64,
                                gpu_mem_percentage: proc.mem_pct as f64,
                                gpu_mem_size_kib: proc.mem_size_kib as usize,
                                ..Default::default()
                            }
                        });
                }
            }
            Err(_e) => {
                gpu_status = GpuStatus::UnknownFailure;
            }
        }
    }

    // If there was a gpu failure, signal it in all the process structures.  This is pretty
    // conservative and increases output data volume, but it means that the information is not lost
    // so long as not all records from this sonar run are filtered out by the front end.

    if gpu_status != GpuStatus::Ok {
        for proc_info in procinfo_by_pid.values_mut() {
            proc_info.gpu_status = gpu_status;
        }
    }

    (procinfo_by_pid, gpu_info)
}

fn singleton_gpu(device: &gpu::Name, gpu_util: u32, mem_util: u32, mem_size: u64) -> GpuProcInfos {
    let mut h = HashMap::new();
    h.insert(
        device.clone(),
        GpuProcInfo {
            device: device.clone(),
            gpu_util,
            gpu_mem: mem_size,
            gpu_mem_util: mem_util,
        },
    );
    h
}

fn aggregate_gpu(
    gpus: &mut GpuProcInfos,
    device: &gpu::Name,
    gpu_util: u32,
    mem_util: u32,
    mem_size: u64,
) {
    gpus.entry(device.clone())
        .and_modify(|e| {
            e.gpu_util += gpu_util;
            e.gpu_mem_util += mem_util;
            e.gpu_mem += mem_size;
        })
        .or_insert(GpuProcInfo {
            device: device.clone(),
            gpu_util,
            gpu_mem_util: mem_util,
            gpu_mem: mem_size,
        });
}

fn aggregate_gpus(gpus: &mut GpuProcInfos, others: &GpuProcInfos) {
    for (name, info) in others {
        aggregate_gpu(gpus, name, info.gpu_util, info.gpu_mem_util, info.gpu_mem);
    }
}

fn rollup_processes(procinfo_by_pid: ProcInfoTable) -> Vec<ProcInfo> {
    // This is a little complicated because processes with job_id 0 or processes that have
    // subprocesses or processes that do not belong to Slurm jobs cannot be rolled up, nor can
    // we roll up processes with different ppid.
    //
    // The reason we cannot roll up processes with job_id 0 is that we don't know that they are
    // related at all - 0 means "no information".
    //
    // The reason we cannot roll up processes with children or processes with different ppids or
    // non-slurm processes is that this would break subsequent processing - it would make it
    // impossible to build a sensible process tree from the sample data.
    //
    // - There is an array `rolledup` of ProcInfo nodes that represent rolled-up data
    //
    // - When the job ID of a process in `procinfo_by_pid` is zero, or a process has children, the
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
    for proc_info in procinfo_by_pid.values() {
        if proc_info.job_id == 0 || proc_info.has_children || !proc_info.is_slurm {
            rolledup.push(proc_info.clone());
        } else {
            let key = (proc_info.job_id, proc_info.ppid, proc_info.command.as_str());
            if let Some(x) = index.get(&key) {
                let p = &mut rolledup[*x];
                p.num_threads += proc_info.num_threads;
                p.cpu_percentage += proc_info.cpu_percentage;
                p.cpu_util += proc_info.cpu_util;
                p.cputime_sec += proc_info.cputime_sec;
                p.mem_percentage += proc_info.mem_percentage;
                p.mem_size_kib += proc_info.mem_size_kib;
                p.rssanon_kib += proc_info.rssanon_kib;
                p.data_read_kib += proc_info.data_read_kib;
                p.data_written_kib += proc_info.data_written_kib;
                p.data_cancelled_kib += proc_info.data_cancelled_kib;
                aggregate_gpus(&mut p.gpus, &proc_info.gpus);
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
}

fn filter_proc(proc_info: &ProcInfo, opts: &PsOptions) -> bool {
    let mut included = false;

    // The logic here is that if any of the inclusion filters are provided, then the set of those
    // that are provided constitute the entire inclusion filter, and the record must pass at least
    // one of those to be included.  Otherwise, when none of the filters are provided then the
    // record is included by default.

    if opts.min_cpu_percent.is_some()
        || opts.min_mem_percent.is_some()
        || opts.min_cpu_time.is_some()
    {
        if let Some(cpu_cutoff_percent) = opts.min_cpu_percent {
            if proc_info.cpu_percentage >= cpu_cutoff_percent {
                included = true;
            }
        }
        if let Some(mem_cutoff_percent) = opts.min_mem_percent {
            if proc_info.mem_percentage >= mem_cutoff_percent {
                included = true;
            }
        }
        if let Some(cpu_cutoff_time) = opts.min_cpu_time {
            if proc_info.cputime_sec >= cpu_cutoff_time {
                included = true;
            }
        }
    } else {
        included = true;
    }

    // The exclusion filters apply after the inclusion filters and the record must pass all of the
    // ones that are provided.

    if opts.exclude_system_jobs && proc_info.is_system_job {
        included = false;
    }
    if !opts.exclude_users.is_empty() && opts.exclude_users.contains(&proc_info.user) {
        included = false;
    }
    if !opts.exclude_commands.is_empty()
        && opts
            .exclude_commands
            .iter()
            .any(|x| proc_info.command.starts_with(x))
    {
        included = false;
    }

    included
}
