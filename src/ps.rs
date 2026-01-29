#![allow(clippy::len_zero)]
#![allow(dead_code)]

use crate::gpu;
use crate::json_tags::*;
use crate::output;
use crate::ps_newfmt::format_newfmt;
use crate::systemapi::{self, DiskInfo};
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

// The PidMap is used for rolled-up pid synthesis.  A rolled-up process - a (job, parent, command)
// triple - is entered into the map when it is first encountered and holds the synthesized pid of
// that process.  The rolled-up process is born dirty.  Whenever it is encountered during subsequent
// sampling it is also marked dirty.  At the end of processing a full set of samples, the map can be
// scanned and clean elements can be removed, following which all the dirty elements that remain
// become clean.  When a pid is needed but not available, a scan of the pid space finds free ones.
//
// The set of active synthesized pids is expected to be small - at most a few thousand elements even
// on very large and busy nodes, and typically much less than that.
//
// Semantically it is desirable that a synthesized pid is not reused within the same job.  It's
// impossible to guarantee that, though it is very, very likely to work out if we manage to reuse
// pids in LRU order.  With a large enough PID space, simply cycling through the space and always
// picking the next available one is likely to be a good approximation to that.
//
// With that in mind: to scan the pid space we create a (typically small) sorted list of the active
// pids and then build a set of ranges of available pids from that, and put those in a pool in
// numerical order, and then refill from the pool, wrapping around when we get to the end.

struct PidMap {
    map: HashMap<ProcessKey, ProcessValue>,
    before_first: u64,         // sentinel, max system pid
    after_last: u64,           // sentinel, at most u64::MAX
    fresh_pid: u64,            // current range min (changes as we allocate pids)
    curr_max: u64,             // current range max
    pid_pool: Vec<(u64, u64)>, // (min, max) of a range, but the max is never u64::MAX; sorted descending.
    dirty: bool,               // value meaning dirty
}

#[derive(Eq, Hash, PartialEq)]
struct ProcessKey {
    job_id: JobID,
    ppid: Pid,
    command: String,
}

struct ProcessValue {
    pid: u64,
    dirty: bool,
}

// These parameters are sensible for a "large enough" pid range, but can be set to other, more
// aggressive, values during testing with an env var.

// The upper limit on the pid range, exclusive.
const PID_LIMIT: u64 = std::u64::MAX;

// Ranges with fewer than this many elements are not retained in the free pool, to keep its size
// manageable.
const MIN_RANGE_SIZE: u64 = 100;

impl PidMap {
    fn new(system: &dyn systemapi::SystemAPI) -> PidMap {
        let limits = get_limits(system);
        PidMap {
            map: HashMap::new(),
            before_first: system.get_pid_max(),
            after_last: limits.pid_limit,
            fresh_pid: system.get_pid_max() + 1,
            curr_max: limits.pid_limit - 1,
            pid_pool: vec![],
            dirty: true,
        }
    }

    fn avail(&self) -> u64 {
        self.pid_pool
            .iter()
            .map(|v| v.1 - v.0 + 1)
            .fold(0, |a, b| a + b)
            + (self.curr_max - self.fresh_pid + 1)
    }

    fn advance(&mut self, system: &dyn systemapi::SystemAPI) {
        self.fresh_pid += 1;
        if self.fresh_pid > self.curr_max {
            match self.pid_pool.pop() {
                Some((low, high)) => {
                    (self.fresh_pid, self.curr_max) = (low, high);
                }
                None => {
                    self.sweep(system);
                }
            }
        }
    }

    // Always purge all clean elements.  This costs a little extra - we could have decided not to
    // purge every time this function is called - but keeps things predictable.  Sweeping always
    // happens on demand, not here.

    fn purge_clean(&mut self) {
        self.map.retain(|_, v| v.dirty == self.dirty);
        self.dirty = !self.dirty;

        let verbose = std::env::var("SONARTEST_ROLLUP_PIDS").is_ok();
        if verbose {
            log::debug!("PID GC: Dirty after purge: {}", self.map.len());
        }
    }

    // The sweeper will set up new fresh_pid/curr_max values.  It does not return if it can't
    // allocate at least one pid.

    fn sweep(&mut self, system: &dyn systemapi::SystemAPI) {
        let limits = get_limits(system);
        let verbose = std::env::var("SONARTEST_ROLLUP_PIDS").is_ok();
        let target = self.fresh_pid;

        if verbose {
            log::debug!("PID GC: Target = {target}");
        }

        self.fresh_pid = 0;
        self.curr_max = 0;
        self.pid_pool.clear();
        let mut xs = self.map.values().map(|v| v.pid).collect::<Vec<u64>>();
        xs.push(self.before_first);
        xs.push(self.after_last);
        xs.sort();
        let mut i = xs.len() - 1;
        while i > 0 {
            // Note we may have high < low now.
            let high = xs[i] - 1;
            let low = xs[i - 1] + 1;
            if high >= low && high - low + 1 >= limits.min_range_size {
                if verbose {
                    log::debug!("PID GC: Recover {low}..{high}");
                }
                self.pid_pool.push((low, high));
            }
            i -= 1;
        }
        if self.pid_pool.is_empty() {
            panic!("PID GC: Empty PID pool");
        }

        if verbose {
            log::debug!("PID GC: Total available after collection {}", self.avail());
        }

        // Now, target points to the next pid to use, so we must pop the pool until we we find a
        // range that covers that value or one that is higher.  If there are no such ranges then we
        // retain all ranges and start at the low one.  This ensures that we cycle through available
        // pids and get a quasi-LRU order for a large enough PID space.

        if target > self.pid_pool[0].1 {
            if verbose {
                log::debug!("PID GC: Wrapped around");
            }
            (self.fresh_pid, self.curr_max) = self.pid_pool.pop().unwrap();
        } else {
            loop {
                (self.fresh_pid, self.curr_max) = self.pid_pool.pop().unwrap();
                if self.curr_max >= target {
                    if verbose {
                        log::debug!("PID GC: Finding {} {}", self.fresh_pid, self.curr_max);
                    }
                    self.fresh_pid = target;
                    break;
                }
                if verbose {
                    log::debug!(
                        "PID GC: Discarding {} {} avail = {}",
                        self.fresh_pid,
                        self.curr_max,
                        self.avail()
                    );
                }
            }
        }

        if verbose {
            log::debug!("PID GC: Actual available after collection {}", self.avail());
        }
    }
}

struct Limits {
    pid_limit: u64,
    min_range_size: u64,
}

#[allow(unused_variables)]
fn get_limits(system: &dyn systemapi::SystemAPI) -> Limits {
    #[allow(unused_mut)]
    let mut limits = Limits {
        pid_limit: PID_LIMIT,
        min_range_size: MIN_RANGE_SIZE,
    };

    #[cfg(debug_assertions)]
    if let Ok(s) = std::env::var("SONARTEST_ROLLUP_PIDS") {
        // SONARTEST_ROLLUP_PIDS should have the form p,r where p is the number of available pids
        // and r is the minimum pid range size to keep after garbage collection.  All are optional.
        let mut xs = s.split(",").map(|v| v.parse::<u64>());
        match xs.next() {
            Some(Ok(v)) => limits.pid_limit = system.get_pid_max() + 1 + v,
            Some(_) | None => {}
        }
        match xs.next() {
            Some(Ok(v)) => limits.min_range_size = v,
            Some(_) | None => {}
        }
    }

    limits
}

#[cfg(feature = "daemon")]
pub struct State<'a> {
    system: &'a dyn systemapi::SystemAPI,
    opts: PsOptions,
    pidmap: PidMap,
}

#[cfg(feature = "daemon")]
impl<'a> State<'a> {
    pub fn new(system: &'a dyn systemapi::SystemAPI, opts: &PsOptions) -> State<'a> {
        State {
            system,
            opts: opts.clone(),
            pidmap: PidMap::new(system),
        }
    }

    pub fn run(&mut self) -> Vec<Vec<u8>> {
        let mut writer = Vec::new();
        do_create_snapshot(&mut writer, self.system, &self.opts, Some(&mut self.pidmap));
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
            do_create_snapshot(writer, system, opts, None);

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
        do_create_snapshot(writer, system, opts, None);
    }
}

fn do_create_snapshot(
    writer: &mut dyn io::Write,
    system: &dyn systemapi::SystemAPI,
    opts: &PsOptions,
    pidmap: Option<&mut PidMap>,
) {
    match collect_sample_data(system, opts) {
        Ok(Some(mut sample_data)) => {
            // Assign PIDs to rolled-up processes?
            if let Some(pidmap) = pidmap {
                let verbose = std::env::var("SONARTEST_ROLLUP_PIDS").is_ok();
                for s in sample_data.process_samples.iter_mut() {
                    if s.rolledup > 0 {
                        let key = ProcessKey {
                            job_id: s.job_id,
                            ppid: s.ppid,
                            command: s.command.to_string(),
                        };
                        let mut advance = false;
                        pidmap
                            .map
                            .entry(key)
                            .and_modify(|e| {
                                // Note this case should only be hit on *subsequent* samples.
                                if verbose {
                                    log::debug!(
                                        "PID synthesis: Old process: {} {} {} {}",
                                        s.job_id,
                                        s.ppid,
                                        s.command,
                                        e.pid
                                    );
                                }
                                s.pid = e.pid as usize;
                                e.dirty = pidmap.dirty;
                            })
                            .or_insert_with(|| {
                                if verbose {
                                    log::debug!(
                                        "PID synthesis: New process: {} {} {} {}",
                                        s.job_id,
                                        s.ppid,
                                        s.command,
                                        pidmap.fresh_pid
                                    );
                                }
                                advance = true;
                                let pid = pidmap.fresh_pid;
                                s.pid = pid as usize;
                                ProcessValue {
                                    pid: pid,
                                    dirty: pidmap.dirty,
                                }
                            });
                        if advance {
                            pidmap.advance(system);
                        }
                    }
                }
                pidmap.purge_clean();
            }
            match opts.fmt {
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
            }
        }
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

#[derive(Clone, Copy, Default, PartialEq)]
pub enum CState {
    #[default]
    Unknown,
    Root,
    Child,
    Not,
}

#[derive(Clone, Default)]
pub struct ProcInfo {
    pub user: String,
    pub command: String,
    pub pid: Pid,
    pub ppid: Pid,
    pub rolledup: usize,
    pub num_threads: usize,
    pub is_system_job: bool,
    pub container_state: CState,
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
    pub disk_samples: Vec<DiskInfo>,
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
    let disk_info = system.compute_disk_information()?;

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

    if opts.exclude_system_jobs {
        procinfo_by_pid = mark_containers(procinfo_by_pid);
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
        disk_samples: disk_info,
        used_memory: memory.total - memory.available,
        load1,
        load5,
        load15,
        runnable_entities,
        existing_entities,
    }))
}

fn mark_containers(mut procinfo_by_pid: ProcInfoTable) -> ProcInfoTable {
    // Pass 1: Mark roots.
    for (_, v) in &mut procinfo_by_pid {
        if is_container_root(v) {
            v.container_state = CState::Root;
        } else if v.ppid == 0 {
            v.container_state = CState::Not;
        }
    }

    // Pass 2: Walk up the tree from every process and propagate state from marked parents to its
    // unmarked children.  Root marking ensures that the walk normally will terminate in a marked
    // parent, but due to how information is collected there's the chance that a ppid will not exist
    // in the table, so we must deal with that.
    //
    // The separate collecting of keys sucks but is a fact of Rust.
    let all = procinfo_by_pid.keys().map(|v| *v).collect::<Vec<Pid>>();
    let mut keys = vec![];
    for k in all {
        keys.clear();
        let mut current = k;
        let mut state = CState::Unknown;
        while let Some(v) = procinfo_by_pid.get(&current) {
            state = v.container_state;
            if state != CState::Unknown {
                break;
            }
            keys.push(current);
            current = v.ppid;
        }
        // Invariant: every node in keys has state Unknown and unless the search terminated in a
        // non-existent parent then the state variable has the (not Unknown) state of the parent of
        // the topmost node in keys.  Note keys may be empty.
        state = match state {
            CState::Unknown => CState::Not,
            CState::Root => CState::Child,
            x => x,
        };
        for k in &keys {
            procinfo_by_pid.get_mut(k).unwrap().container_state = state;
        }
    }

    procinfo_by_pid
}

// Any subprocess of a process whose name starts with "containerd" whose parent is init is a process
// running in a docker container and it should be marked as such.  It's probably possible to get
// false positives for this.
fn is_container_root(v: &ProcInfo) -> bool {
    v.command.starts_with("containerd") && v.ppid == 1
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
                rolledup.push(ProcInfo {
                    pid: 0,
                    ..proc_info.clone()
                });
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

    if opts.exclude_system_jobs
        && proc_info.is_system_job
        && proc_info.container_state != CState::Child
    {
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
