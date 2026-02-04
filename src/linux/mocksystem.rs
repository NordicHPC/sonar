use crate::gpu;
use crate::gpu::mockgpu;
use crate::jobsapi;
use crate::json_tags;
use crate::linux::{procfs, system};
use crate::systemapi;
use crate::time;

use std::cell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path;

#[derive(Default)]
pub struct Builder {
    proc_files: Option<HashMap<String, String>>,
    node_files: Option<HashMap<String, String>>,
    pids: Option<Vec<(usize, u32)>>,
    threads: Option<HashMap<usize, Vec<(usize, u32)>>>,
    users: Option<HashMap<u32, String>>,
    now: Option<u64>,
    boot_time: Option<u64>,
    timestamp: Option<String>,
    hostname: Option<String>,
    cluster: Option<String>,
    node_domain: Option<Vec<String>>,
    version: Option<String>,
    os_name: Option<String>,
    os_release: Option<String>,
    architecture: Option<String>,
    jm: Option<Box<dyn jobsapi::JobManager>>,
    cards: cell::RefCell<Vec<gpu::Card>>,
}

#[allow(dead_code)]
impl Builder {
    pub fn new() -> Builder {
        Builder {
            ..Default::default()
        }
    }

    // Files below /proc
    pub fn with_proc_files(self, files: HashMap<String, String>) -> Builder {
        Builder {
            proc_files: Some(files),
            ..self
        }
    }

    // Files below /sys/devices/system/node
    pub fn with_node_files(self, files: HashMap<String, String>) -> Builder {
        Builder {
            node_files: Some(files),
            ..self
        }
    }

    pub fn with_pids(self, pids: Vec<(usize, u32)>) -> Builder {
        Builder {
            pids: Some(pids),
            ..self
        }
    }

    pub fn with_threads(self, threads: HashMap<usize, Vec<(usize, u32)>>) -> Builder {
        Builder {
            threads: Some(threads),
            ..self
        }
    }

    pub fn with_users(self, users: HashMap<u32, String>) -> Builder {
        Builder {
            users: Some(users),
            ..self
        }
    }

    pub fn with_now(self, now: u64) -> Builder {
        Builder {
            now: Some(now),
            ..self
        }
    }

    pub fn with_timestamp(self, timestamp: &str) -> Builder {
        Builder {
            timestamp: Some(timestamp.to_string()),
            ..self
        }
    }

    pub fn with_hostname(self, hostname: &str) -> Builder {
        Builder {
            hostname: Some(hostname.to_string()),
            ..self
        }
    }

    pub fn with_cluster(self, cluster: &str) -> Builder {
        Builder {
            cluster: Some(cluster.to_string()),
            ..self
        }
    }

    pub fn with_node_domain(self, domain: &[String]) -> Builder {
        Builder {
            node_domain: Some(domain.iter().map(|x| x.clone()).collect::<Vec<String>>()),
            ..self
        }
    }

    pub fn with_os(self, name: &str, release: &str) -> Builder {
        Builder {
            os_name: Some(name.to_string()),
            os_release: Some(release.to_string()),
            ..self
        }
    }

    pub fn with_architecture(self, architecture: &str) -> Builder {
        Builder {
            architecture: Some(architecture.to_string()),
            ..self
        }
    }

    pub fn with_jobmanager(self, jm: Box<dyn jobsapi::JobManager>) -> Builder {
        Builder {
            jm: Some(jm),
            ..self
        }
    }

    pub fn with_version(self, version: &str) -> Builder {
        Builder {
            version: Some(version.to_string()),
            ..self
        }
    }

    pub fn with_card(self, c: gpu::Card) -> Builder {
        self.cards.borrow_mut().push(c);
        self
    }

    pub fn with_boot_time(self, boot_time: u64) -> Builder {
        Builder {
            boot_time: Some(boot_time),
            ..self
        }
    }

    pub fn freeze(self) -> MockSystem {
        MockSystem {
            version: if let Some(x) = self.version {
                x
            } else {
                "0.0.0".to_string()
            },
            timestamp: if let Some(x) = self.timestamp {
                x
            } else {
                time::now_iso8601()
            },
            jm: if let Some(x) = self.jm {
                x
            } else {
                Box::new(jobsapi::NoJobManager::new())
            },
            hostname: if let Some(x) = self.hostname {
                x
            } else {
                "no.host.com".to_string()
            },
            cluster: if let Some(x) = self.cluster {
                x
            } else {
                "no.cluster.com".to_string()
            },
            node_domain: self.node_domain,
            os_name: if let Some(x) = self.os_name {
                x
            } else {
                "unknown-os".to_string()
            },
            os_release: if let Some(x) = self.os_release {
                x
            } else {
                "unknown-release".to_string()
            },
            architecture: if let Some(x) = self.architecture {
                x
            } else {
                "unknown-architecture".to_string()
            },
            fs: {
                let proc_files = self.proc_files.unwrap_or_default();
                let pids = self.pids.unwrap_or_default();
                let threads = self.threads.unwrap_or_default();
                MockFS {
                    proc_files,
                    pids,
                    threads,
                }
            },
            node_files: self.node_files.unwrap_or_default(),
            now: if let Some(x) = self.now {
                x
            } else {
                time::unix_now()
            },
            boot_time: if let Some(x) = self.boot_time {
                x
            } else {
                json_tags::EPOCH_TIME_BASE + 1
            },
            gpus: mockgpu::MockGpuAPI::new(self.cards.take()),
            pid: 1337,
            ticks_per_sec: 100,
            pagesz: 4,
            users: self.users.unwrap_or_default(),
        }
    }
}

pub struct MockSystem {
    timestamp: String,
    jm: Box<dyn jobsapi::JobManager>,
    hostname: String,
    cluster: String,
    node_domain: Option<Vec<String>>,
    os_name: String,
    os_release: String,
    architecture: String,
    fs: MockFS,
    node_files: HashMap<String, String>,
    gpus: mockgpu::MockGpuAPI,
    users: HashMap<u32, String>,
    pid: u32,
    version: String,
    ticks_per_sec: usize,
    pagesz: usize,
    now: u64,
    boot_time: u64,
}

impl MockSystem {
    pub fn get_procfs(&self) -> &MockFS {
        &self.fs
    }
}

impl systemapi::SystemAPI for MockSystem {
    fn get_version(&self) -> String {
        self.version.clone()
    }

    fn get_timestamp(&self) -> String {
        self.timestamp.clone()
    }

    fn get_hostname(&self) -> String {
        self.hostname.clone()
    }

    fn get_cluster(&self) -> String {
        self.cluster.clone()
    }

    fn get_node_domain(&self) -> &Option<Vec<String>> {
        &self.node_domain
    }

    fn get_os_name(&self) -> String {
        self.os_name.clone()
    }

    fn get_os_release(&self) -> String {
        self.os_release.clone()
    }

    fn get_architecture(&self) -> String {
        self.architecture.clone()
    }

    fn get_gpus(&self) -> &dyn gpu::GpuAPI {
        &self.gpus
    }

    fn get_jobs(&self) -> &dyn jobsapi::JobManager {
        &*self.jm
    }

    fn get_pid(&self) -> u32 {
        self.pid
    }

    fn get_clock_ticks_per_sec(&self) -> usize {
        self.ticks_per_sec
    }

    fn get_page_size_in_kib(&self) -> usize {
        self.pagesz
    }

    fn get_now_in_secs_since_epoch(&self) -> u64 {
        self.now
    }

    fn get_boot_time_in_secs_since_epoch(&self) -> u64 {
        self.boot_time
    }

    fn get_cpu_info(&self) -> Result<systemapi::CpuInfo, String> {
        procfs::get_cpu_info(&self.fs)
    }

    fn get_memory_in_kib(&self) -> Result<systemapi::Memory, String> {
        procfs::get_memory_in_kib(&self.fs)
    }

    fn get_numa_distances(&self) -> Result<Vec<Vec<u32>>, String> {
        system::get_numa_distances(self)
    }

    fn get_pid_max(&self) -> u64 {
        4194304
    }

    fn compute_node_information(&self) -> Result<(u64, Vec<u64>), String> {
        procfs::compute_node_information(self, &self.fs)
    }

    fn compute_disk_information(&self) -> Result<Vec<systemapi::DiskInfo>, String> {
        procfs::compute_disk_information(&self.fs)
    }

    fn compute_loadavg(&self) -> Result<(f64, f64, f64, u64, u64), String> {
        procfs::compute_loadavg(&self.fs)
    }

    fn compute_process_information(&self) -> Result<HashMap<usize, systemapi::Process>, String> {
        procfs::compute_process_information(self, &self.fs)
    }

    fn compute_cpu_utilization(
        &self,
        processes: &HashMap<usize, systemapi::Process>,
        wait_time_ms: usize,
    ) -> Result<Vec<(usize, f64)>, String> {
        procfs::compute_cpu_utilization(self, &self.fs, processes, wait_time_ms)
    }

    fn compute_slurm_job_id(&self, _pid: usize) -> Option<usize> {
        None
    }

    fn compute_user_by_uid(&self, uid: u32) -> Option<String> {
        self.users.get(&uid).map(|s| s.clone())
    }

    fn create_lock_file(&self, _p: &path::Path) -> io::Result<fs::File> {
        panic!("Not in use yet");
    }

    fn remove_lock_file(&self, _p: &path::Path) -> io::Result<()> {
        panic!("Not in use yet");
    }

    fn read_node_file_to_string(&self, filename: &str) -> io::Result<String> {
        match self.node_files.get(filename) {
            Some(contents) => Ok(contents.clone()),
            None => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }

    fn run_sacct(
        &self,
        _job_states: &[&str],
        _field_names: &[&str],
        _from: &str,
        _to: &str,
    ) -> Result<String, String> {
        Ok("".to_string()) // Not in use yet
    }

    fn run_scontrol(&self) -> Result<String, String> {
        Ok("".to_string()) // Not in use yet
    }

    fn compute_cluster_kind(&self) -> Option<systemapi::ClusterKind> {
        None
    }

    fn compute_cluster_partitions(&self) -> Result<Vec<(String, String)>, String> {
        Ok(vec![]) // Not in use yet
    }

    fn compute_cluster_nodes(&self) -> Result<Vec<(String, String)>, String> {
        Ok(vec![]) // Not in use yet
    }

    fn compute_node_topo_svg(&self) -> Result<Option<String>, String> {
        Ok(None) // Not in use yet
    }

    fn compute_node_topo_text(&self) -> Result<Option<String>, String> {
        Ok(None) // Not in use yet
    }

    fn handle_interruptions(&self) {
        // Nothing yet
    }

    fn is_interrupted(&self) -> bool {
        // Nothing yet
        false
    }
}

pub struct MockFS {
    proc_files: HashMap<String, String>,
    pids: Vec<(usize, u32)>,
    threads: HashMap<usize, Vec<(usize, u32)>>,
}

impl procfs::ProcfsAPI for MockFS {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        match self.proc_files.get(path) {
            Some(s) => Ok(s.clone()),
            None => Err(format!("Unable to read /proc/{path}")),
        }
    }

    fn read_numeric_file_names(&self, path: &str) -> Result<Vec<(usize, u32)>, String> {
        if path == "" {
            Ok(self.pids.clone())
        } else {
            // We require the path to be <pid>/task, so parse the pid
            match path.split_once('/') {
                Some((a, _)) => match a.parse::<usize>() {
                    Ok(pid) => match self.threads.get(&pid) {
                        Some(v) => Ok(v.clone()),
                        None => Ok(vec![(0, 0)]),
                    },
                    Err(_) => Err(format!("Unknown {path}")),
                },
                None => Err(format!("Unknown {path}")),
            }
        }
    }
}
