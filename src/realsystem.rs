use crate::command;
use crate::gpuapi;
use crate::hostname;
use crate::interrupt;
use crate::jobsapi;
use crate::procfs;
use crate::procfsapi;
use crate::realgpu;
use crate::realprocfs;
use crate::systemapi;
use crate::time;
use crate::users;
use crate::util;

use std::fs;
use std::io;
use std::path;

// 3 minutes ought to be enough for anyone.
const SACCT_TIMEOUT_S: u64 = 180;

pub struct RealSystemBuilder {
    jm: Option<Box<dyn jobsapi::JobManager>>,
    cluster: String,
}

impl RealSystemBuilder {
    #[allow(dead_code)]
    pub fn with_cluster(self, cluster: &str) -> RealSystemBuilder {
        RealSystemBuilder {
            cluster: cluster.to_string(),
            ..self
        }
    }

    pub fn with_jobmanager(self, jm: Box<dyn jobsapi::JobManager>) -> RealSystemBuilder {
        RealSystemBuilder {
            jm: Some(jm),
            ..self
        }
    }

    pub fn freeze(self) -> Result<RealSystem, String> {
        let fs = realprocfs::RealProcFS::new();
        let boot_time = procfs::get_boot_time(&fs)?;
        Ok(RealSystem {
            timestamp: time::now_iso8601(),
            hostname: hostname::get(),
            cluster: self.cluster,
            jm: if let Some(x) = self.jm {
                x
            } else {
                Box::new(jobsapi::NoJobManager::new())
            },
            fs,
            gpus: realgpu::RealGpu::new(),
            now: time::unix_now(),
            boot_time,
        })
    }
}

pub struct RealSystem {
    timestamp: String,
    hostname: String,
    cluster: String,
    fs: realprocfs::RealProcFS,
    gpus: realgpu::RealGpu,
    jm: Box<dyn jobsapi::JobManager>,
    now: u64,
    boot_time: u64,
}

impl RealSystem {
    pub fn new() -> RealSystemBuilder {
        RealSystemBuilder { jm: None, cluster: "".to_string() }
    }
}

impl systemapi::SystemAPI for RealSystem {
    fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn get_timestamp(&self) -> String {
        self.timestamp.clone()
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

    fn get_procfs(&self) -> &dyn procfsapi::ProcfsAPI {
        &self.fs
    }

    fn get_gpus(&self) -> &dyn gpuapi::GpuAPI {
        &self.gpus
    }

    fn get_jobs(&self) -> &dyn jobsapi::JobManager {
        &*self.jm
    }

    fn get_pid(&self) -> u32 {
        std::process::id()
    }

    fn get_boot_time(&self) -> u64 {
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
        self.now
    }

    fn user_by_uid(&self, uid: u32) -> Option<String> {
        users::get_user_by_uid(uid).map(|u| u.to_string_lossy().to_string())
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
        match command::safe_command(
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
        ) {
            Err(e) => Err(format!("sacct failed: {:?}", e)),
            Ok(sacct_output) => Ok(sacct_output),
        }
    }

    fn handle_interruptions(&self) {
        interrupt::handle_interruptions();
    }

    fn is_interrupted(&self) -> bool {
        interrupt::is_interrupted()
    }
}
