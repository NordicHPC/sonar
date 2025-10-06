use crate::gpu;
use crate::jobsapi;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path;

pub enum ClusterKind {
    Slurm,
}

// Naming: The `get_` methods always return the same values for every call.  The `compute_` methods
// all recompute the data and the values may change.
pub trait SystemAPI {
    fn get_version(&self) -> String;
    fn get_timestamp(&self) -> String;
    fn get_cluster(&self) -> String;
    fn get_hostname(&self) -> String;
    fn get_os_name(&self) -> String;
    fn get_os_release(&self) -> String;
    fn get_architecture(&self) -> String;
    fn get_clock_ticks_per_sec(&self) -> usize;
    fn get_page_size_in_kib(&self) -> usize;
    fn get_now_in_secs_since_epoch(&self) -> u64;
    fn get_pid(&self) -> u32;
    fn get_boot_time_in_secs_since_epoch(&self) -> u64;
    fn get_gpus(&self) -> &dyn gpu::GpuAPI;
    fn get_jobs(&self) -> &dyn jobsapi::JobManager;
    fn get_cpu_info(&self) -> Result<CpuInfo, String>;
    fn get_memory_in_kib(&self) -> Result<Memory, String>;
    fn get_numa_distances(&self) -> Result<Vec<Vec<u32>>, String>;

    // CPU usage data: total cpu seconds and per-cpu seconds.
    fn compute_node_information(&self) -> Result<(u64, Vec<u64>), String>;

    // 1m, 5m, 15m load avg + current runnable and existing entities
    fn compute_loadavg(&self) -> Result<(f64, f64, f64, u64, u64), String>;

    // Return a hashmap of structures with process data, keyed by pid.  Pids uniquely tag the
    // records.  Also return a vector mapping pid to total cpu ticks for the process.
    fn compute_process_information(
        &self,
    ) -> Result<(HashMap<usize, Process>, Vec<(usize, u64)>), String>;

    // Given the per-process CPU time computed by compute_process_information, and a time to wait,
    // wait for that time and then read the CPU time again.  The sampled process CPU utilization is
    // the delta of CPU time divided by the delta of time, 1.0 = 100% of one core.
    fn compute_cpu_utilization(
        &self,
        per_pid_cpu_ticks: &[(usize, u64)],
        wait_time_ms: usize,
    ) -> Result<Vec<(usize, f64)>, String>;

    // This returns Some(n) where n > 0 if we could parse the job ID, Some(0) if the API is
    // available but the ID is not obtainable, or None otherwise.  Thus None is a signal to fall
    // back to other (non-Slurm) mechanisms.
    fn compute_slurm_job_id(&self, pid: usize) -> Option<usize>;

    // Try to figure out the user's name from system tables, this may be an expensive operation.
    // There's a tiny risk that the answer could change between two calls (if a user were added
    // and/or removed).
    fn compute_user_by_uid(&self, uid: u32) -> Option<String>;

    // Run sacct and return its output.  The arguments are passed on to sacct: `job_states` to `-s`,
    // `field_names` to `-o`, `from` to `-S` and `to` to `-E`.  This is only defined for state and
    // field names that exist, and for properly slurm-formatted dates.
    fn run_sacct(
        &self,
        job_states: &[&str],
        field_names: &[&str],
        from: &str,
        to: &str,
    ) -> Result<String, String>;

    fn compute_cluster_kind(&self) -> Option<ClusterKind>;

    // On a batch system, return a vector of partition name and standard-format compressed nodelist.
    fn compute_cluster_partitions(&self) -> Result<Vec<(String, String)>, String>;

    // On a batch system, return a vector of standard-format compressed nodelist and state name
    // list.  State names are currently batch system specific.
    fn compute_cluster_nodes(&self) -> Result<Vec<(String, String)>, String>;

    // `create_lock_file` creates it atomically if it does not exist, returning Ok if so; if it does
    // exist, returns Err(io::ErrorKind::AlreadyExists), otherwise some other Err.
    // `remove_lock_file` unconditionally tries to remove the file, returning some Err if it fails.
    fn create_lock_file(&self, p: &path::PathBuf) -> io::Result<fs::File>;
    fn remove_lock_file(&self, p: path::PathBuf) -> io::Result<()>;

    // `handle_interruptions` enables interrupt checking; `is_interrupted` returns true if an
    // interrupt has been received.  The specific interrupt signals are system-dependent; see the
    // implementations for more.
    fn handle_interruptions(&self);
    fn is_interrupted(&self) -> bool;
}

#[derive(PartialEq, Debug)]
pub struct Process {
    pub pid: usize,
    pub ppid: usize,
    pub pgrp: usize,
    pub uid: usize,
    pub user: String, // _noinfo_<uid> if name unobtainable
    pub cpu_pct: f64, // Cumulative, not very useful but sonalyze uses it
    pub mem_pct: f64,
    pub cpu_util: f64, // Sample (over a short time period), slurm-monitor uses it
    pub cputime_sec: usize,
    pub mem_size_kib: usize,
    pub data_read_kib: usize,
    pub data_written_kib: usize,
    pub data_cancelled_kib: usize,
    pub rssanon_kib: usize,
    pub command: String,
    pub has_children: bool,
    pub num_threads: usize, // including main thread
}

// Figures in KB.
#[derive(Clone)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
}

#[derive(Clone)]
pub struct CpuInfo {
    pub sockets: i32,
    pub cores_per_socket: i32,
    pub threads_per_core: i32,
    pub cores: Vec<CoreInfo>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct CoreInfo {
    pub model_name: String,
    pub logical_index: i32,
    pub physical_index: i32,
}
