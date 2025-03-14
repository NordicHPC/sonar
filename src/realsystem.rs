use crate::command;
use crate::gpuapi;
use crate::hostname;
use crate::interrupt;
use crate::jobsapi;
use crate::procfsapi;
use crate::realgpu;
use crate::realprocfs;
use crate::systemapi;
use crate::time;
use crate::users;

use std::cell::{Cell, RefCell};
use std::fs;
use std::io;
use std::path;

// 3 minutes ought to be enough for anyone.
const SACCT_TIMEOUT_S: u64 = 180;

pub struct RealSystemBuilder {
    jm: Option<Box<dyn jobsapi::JobManager>>,
}

impl RealSystemBuilder {
    pub fn with_jobmanager(self, jm: Box<dyn jobsapi::JobManager>) -> RealSystemBuilder {
        RealSystemBuilder {
            jm: Some(jm),
            ..self
        }
    }

    pub fn freeze(self) -> RealSystem {
        RealSystem {
            hostname: hostname::get(),
            jm: if let Some(x) = self.jm {
                x
            } else {
                Box::new(jobsapi::NoJobManager::new())
            },
            fs: realprocfs::RealProcFS::new(),
            gpus: realgpu::RealGpu::new(),
            timestamp: RefCell::new(time::now_iso8601()),
            now: Cell::new(time::unix_now()),
        }
    }
}

pub struct RealSystem {
    hostname: String,
    fs: realprocfs::RealProcFS,
    gpus: realgpu::RealGpu,
    jm: Box<dyn jobsapi::JobManager>,
    timestamp: RefCell<String>,
    now: Cell<u64>,
}

impl RealSystem {
    pub fn new() -> RealSystemBuilder {
        RealSystemBuilder { jm: None }
    }
}

impl systemapi::SystemAPI for RealSystem {
    fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn get_timestamp(&self) -> String {
        self.timestamp.borrow().clone()
    }

    fn get_hostname(&self) -> String {
        self.hostname.clone()
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

impl RealSystem {
    // We want the time to be stable (ie unchanging if we read it multiple times), but that also
    // means that when it must move we must move it specifically.  This is not yet part of the
    // SystemAPI because that's not been necessary.
    #[cfg(feature = "daemon")]
    pub fn update_time(&self) {
        self.timestamp.replace(time::now_iso8601());
        self.now.set(time::unix_now());
    }
}
