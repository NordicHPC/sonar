use crate::command;
use crate::gpu;
use crate::gpu::realgpu;
use crate::hostname;
use crate::jobsapi;
use crate::linux::procfs;
use crate::linux::slurm;
#[cfg(debug_assertions)]
use crate::log;
use crate::systemapi;
use crate::time;
use crate::users;
use crate::util;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::linux::fs::MetadataExt;
use std::path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::signal;
use signal_hook::flag;

// 3 minutes ought to be enough for anyone.
const SACCT_TIMEOUT_S: u64 = 180;

// sinfo will normally be very quick
const SINFO_TIMEOUT_S: u64 = 10;

pub struct Builder {
    jm: Option<Box<dyn jobsapi::JobManager>>,
    cluster: String,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            jm: None,
            cluster: "".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_cluster(self, cluster: &str) -> Builder {
        Builder {
            cluster: cluster.to_string(),
            ..self
        }
    }

    pub fn with_jobmanager(self, jm: Box<dyn jobsapi::JobManager>) -> Builder {
        Builder {
            jm: Some(jm),
            ..self
        }
    }

    pub fn freeze(self) -> Result<System, String> {
        let fs = RealProcFS {};
        let boot_time = procfs::get_boot_time_in_secs_since_epoch(&fs)?;
        Ok(System {
            hostname: hostname::get(),
            cluster: self.cluster,
            jm: if let Some(x) = self.jm {
                x
            } else {
                Box::new(jobsapi::NoJobManager::new())
            },
            fs,
            gpus: realgpu::RealGpu::new(),
            timestamp: RefCell::new(time::now_iso8601()),
            now: Cell::new(time::unix_now()),
            boot_time,
            interrupted: Arc::new(AtomicBool::new(false)),
        })
    }
}

#[cfg(target_arch = "x86_64")]
const ARCHITECTURE: &str = "x86_64";

#[cfg(target_arch = "aarch64")]
const ARCHITECTURE: &'static str = "aarch64";

// Otherwise, `ARCHITECTURE` will be undefined and we'll get a compile error; rustc is good about
// notifying the user that there are ifdef'd cases that are inactive.

pub struct System {
    hostname: String,
    cluster: String,
    fs: RealProcFS,
    gpus: realgpu::RealGpu,
    jm: Box<dyn jobsapi::JobManager>,
    timestamp: RefCell<String>,
    now: Cell<u64>,
    boot_time: u64,
    interrupted: Arc<AtomicBool>,
}

impl System {
    // We want the time to be stable (ie unchanging if we read it multiple times), but that also
    // means that when it must move we must move it specifically.  This is not yet part of the
    // SystemAPI because that's not been necessary.
    #[cfg(feature = "daemon")]
    pub fn update_time(&self) {
        self.timestamp.replace(time::now_iso8601());
        self.now.set(time::unix_now());
    }
}

impl systemapi::SystemAPI for System {
    fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn get_timestamp(&self) -> String {
        self.timestamp.borrow().clone()
    }

    fn get_cluster(&self) -> String {
        self.cluster.clone()
    }

    fn get_hostname(&self) -> String {
        self.hostname.clone()
    }

    fn get_os_name(&self) -> String {
        unsafe {
            let mut utsname: libc::utsname = std::mem::zeroed();
            if libc::uname(&mut utsname) != 0 {
                return "".to_string();
            }
            util::cstrdup(&utsname.sysname)
        }
    }

    fn get_os_release(&self) -> String {
        unsafe {
            let mut utsname: libc::utsname = std::mem::zeroed();
            if libc::uname(&mut utsname) != 0 {
                return "".to_string();
            }
            util::cstrdup(&utsname.release)
        }
    }

    fn get_architecture(&self) -> String {
        ARCHITECTURE.to_string()
    }

    fn get_gpus(&self) -> &dyn gpu::GpuAPI {
        &self.gpus
    }

    fn get_jobs(&self) -> &dyn jobsapi::JobManager {
        &*self.jm
    }

    fn get_pid(&self) -> u32 {
        std::process::id()
    }

    fn get_boot_time_in_secs_since_epoch(&self) -> u64 {
        self.boot_time
    }

    fn get_clock_ticks_per_sec(&self) -> usize {
        // We're assuming this never changes while the system is running.
        unsafe { libc::sysconf(libc::_SC_CLK_TCK) as usize }
    }

    fn get_page_size_in_kib(&self) -> usize {
        // We're assuming this never changes while the system is running.
        (unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }) / 1024
    }

    fn get_now_in_secs_since_epoch(&self) -> u64 {
        self.now.get()
    }

    fn get_cpu_info(&self) -> Result<systemapi::CpuInfo, String> {
        procfs::get_cpu_info(&self.fs)
    }

    fn get_memory_in_kib(&self) -> Result<systemapi::Memory, String> {
        procfs::get_memory_in_kib(&self.fs)
    }

    fn compute_node_information(&self) -> Result<(u64, Vec<u64>), String> {
        procfs::compute_node_information(self, &self.fs)
    }

    fn compute_loadavg(&self) -> Result<(f64, f64, f64, u64, u64), String> {
        procfs::compute_loadavg(&self.fs)
    }

    fn compute_process_information(
        &self,
    ) -> Result<(HashMap<usize, systemapi::Process>, Vec<(usize, u64)>), String> {
        procfs::compute_process_information(self, &self.fs)
    }

    fn compute_cpu_utilization(
        &self,
        per_pid_cpu_ticks: &[(usize, u64)],
        wait_time_ms: usize,
    ) -> Result<Vec<(usize, f64)>, String> {
        procfs::compute_cpu_utilization(self, &self.fs, per_pid_cpu_ticks, wait_time_ms)
    }

    fn compute_slurm_job_id(&self, pid: usize) -> Option<usize> {
        slurm::get_job_id(&self.fs, pid)
    }

    fn compute_user_by_uid(&self, uid: u32) -> Option<String> {
        users::lookup_user_by_uid(uid).map(|u| u.to_string_lossy().to_string())
    }

    fn create_lock_file(&self, p: &path::PathBuf) -> io::Result<fs::File> {
        fs::File::options().write(true).create_new(true).open(p)
    }

    fn remove_lock_file(&self, p: path::PathBuf) -> io::Result<()> {
        fs::remove_file(p)
    }

    fn run_sacct(
        &self,
        job_states: &[&str],
        field_names: &[&str],
        from: &str,
        to: &str,
    ) -> Result<String, String> {
        runit(
            "sacct",
            &[
                "-aP",
                "-s",
                &job_states.join(","),
                "--noheader",
                "-o",
                &field_names.join(","),
                "-S",
                from,
                "-E",
                to,
            ],
            SACCT_TIMEOUT_S,
        )
    }

    // Whether we try to run sinfo or run some code to look for the program in the path probably
    // does not matter, and the former is easier.  We could/should cache this value in case the
    // client wants it repeatedly.

    fn compute_cluster_kind(&self) -> Option<systemapi::ClusterKind> {
        match runit("sinfo", &["--usage"], SINFO_TIMEOUT_S) {
            Ok(_) => Some(systemapi::ClusterKind::Slurm),
            Err(_) => None,
        }
    }

    // Basically the `cluster` operations wrap `sinfo`:
    //
    //  - Run sinfo to list partitions.
    //  - Run sinfo to get a list of nodes broken down by partition and their state.
    //
    // The same could have been had in a different form by:
    //
    //  scontrol -o show nodes
    //  scontrol -o show partitions
    //
    // Anyway, we emit a list of partitions with their nodes and a list of nodes with their states.

    fn compute_cluster_partitions(&self) -> Result<Vec<(String, String)>, String> {
        twofields(runit(
            "sinfo",
            &["-h", "-a", "-O", "Partition:|,NodeList:|"],
            SINFO_TIMEOUT_S,
        )?)
    }

    fn compute_cluster_nodes(&self) -> Result<Vec<(String, String)>, String> {
        twofields(runit(
            "sinfo",
            &["-h", "-a", "-e", "-O", "NodeList:|,StateComplete:|"],
            SINFO_TIMEOUT_S,
        )?)
    }

    // Assuming no bugs, the interesting interrupt signals are SIGHUP, SIGTERM, SIGINT, and SIGQUIT.
    // Of these, only SIGHUP and SIGTERM are really interesting because they are sent by the OS or
    // by job control (and will often be followed by SIGKILL if not honored within some reasonable
    // time); INT/QUIT are sent by a user in response to keyboard action and more typical during
    // development/debugging.

    fn handle_interruptions(&self) {
        let _ = flag::register(signal::SIGTERM, Arc::clone(&self.interrupted));
        let _ = flag::register(signal::SIGHUP, Arc::clone(&self.interrupted));
    }

    #[cfg(not(debug_assertions))]
    fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    #[cfg(debug_assertions)]
    fn is_interrupted(&self) -> bool {
        if std::env::var("SONARTEST_WAIT_INTERRUPT").is_ok() {
            std::thread::sleep(std::time::Duration::new(10, 0));
        }
        let flag = self.interrupted.load(Ordering::Relaxed);
        if flag {
            // Test cases depend on this exact output.
            log::info("Interrupt flag was set!")
        }
        flag
    }
}

fn runit(cmd: &str, args: &[&str], timeout: u64) -> Result<String, String> {
    match command::safe_command(cmd, args, timeout) {
        Ok(s) => Ok(s),
        Err(command::CmdError::CouldNotStart(e)) => Err(e),
        Err(command::CmdError::Failed(e)) => Err(e),
        Err(command::CmdError::Hung(e)) => Err(e),
        Err(command::CmdError::InternalError(e)) => Err(e),
    }
}

fn twofields(text: String) -> Result<Vec<(String, String)>, String> {
    let mut v = vec![];
    for l in text.lines() {
        let mut fields = l.split('|');
        let a = fields.next().ok_or("Bad sinfo output")?;
        let b = fields.next().ok_or("Bad sinfo output")?;
        v.push((a.to_string(), b.to_string()));
    }
    Ok(v)
}

struct RealProcFS {}

impl procfs::ProcfsAPI for RealProcFS {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        let filename = format!("/proc/{path}");
        match fs::read_to_string(path::Path::new(&filename)) {
            Ok(s) => Ok(s),
            Err(_) => Err(format!("Unable to read {filename}")),
        }
    }

    fn read_numeric_file_names(&self, path: &str) -> Result<Vec<(usize, u32)>, String> {
        let mut pids = vec![];
        let dir = if path == "" {
            "/proc".to_string()
        } else {
            format!("/proc/{path}/")
        };
        if let Ok(dir) = fs::read_dir(&dir) {
            for dirent in dir.flatten() {
                if let Ok(meta) = dirent.metadata() {
                    let owner = meta.st_uid();
                    if let Some(name) = dirent.path().file_name() {
                        if let Ok(name) = name.to_string_lossy().parse::<usize>() {
                            pids.push((name, owner));
                        }
                    }
                }
            }
        } else {
            return Err("Could not open {dir}".to_string());
        };
        Ok(pids)
    }
}
