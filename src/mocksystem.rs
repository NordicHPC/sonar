use crate::gpuapi;
use crate::jobsapi;
use crate::mockfs;
use crate::mockgpu;
use crate::procfsapi;
use crate::systemapi;
use crate::time;

use std::cell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path;

#[derive(Default)]
pub struct MockSystemBuilder {
    files: Option<HashMap<String, String>>,
    pids: Option<Vec<(usize, u32)>>,
    users: Option<HashMap<u32, String>>,
    now: Option<u64>,
    timestamp: Option<String>,
    hostname: Option<String>,
    version: Option<String>,
    jm: Option<Box<dyn jobsapi::JobManager>>,
    cards: cell::RefCell<Vec<gpuapi::Card>>,
}

#[allow(dead_code)]
impl MockSystemBuilder {
    pub fn with_files(self, files: HashMap<String, String>) -> MockSystemBuilder {
        MockSystemBuilder {
            files: Some(files),
            ..self
        }
    }

    pub fn with_pids(self, pids: Vec<(usize, u32)>) -> MockSystemBuilder {
        MockSystemBuilder {
            pids: Some(pids),
            ..self
        }
    }

    pub fn with_users(self, users: HashMap<u32, String>) -> MockSystemBuilder {
        MockSystemBuilder {
            users: Some(users),
            ..self
        }
    }

    pub fn with_now(self, now: u64) -> MockSystemBuilder {
        MockSystemBuilder {
            now: Some(now),
            ..self
        }
    }

    pub fn with_timestamp(self, timestamp: &str) -> MockSystemBuilder {
        MockSystemBuilder {
            timestamp: Some(timestamp.to_string()),
            ..self
        }
    }

    pub fn with_hostname(self, hostname: &str) -> MockSystemBuilder {
        MockSystemBuilder {
            hostname: Some(hostname.to_string()),
            ..self
        }
    }

    pub fn with_jobmanager(self, jm: Box<dyn jobsapi::JobManager>) -> MockSystemBuilder {
        MockSystemBuilder {
            jm: Some(jm),
            ..self
        }
    }

    pub fn with_version(self, version: &str) -> MockSystemBuilder {
        MockSystemBuilder {
            version: Some(version.to_string()),
            ..self
        }
    }

    pub fn with_card(self, c: gpuapi::Card) -> MockSystemBuilder {
        self.cards.borrow_mut().push(c);
        self
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
            fs: {
                let files = if let Some(x) = self.files {
                    x
                } else {
                    HashMap::new()
                };
                let pids = if let Some(x) = self.pids { x } else { vec![] };
                mockfs::MockFS::new(files, pids)
            },
            now: if let Some(x) = self.now {
                x
            } else {
                time::unix_now()
            },
            gpus: mockgpu::MockGpuAPI::new(self.cards.take()),
            pid: 1337,
            ticks_per_sec: 100,
            pagesz: 4,
            users: if let Some(x) = self.users {
                x
            } else {
                HashMap::new()
            },
        }
    }
}

pub struct MockSystem {
    timestamp: String,
    jm: Box<dyn jobsapi::JobManager>,
    hostname: String,
    fs: mockfs::MockFS,
    gpus: mockgpu::MockGpuAPI,
    users: HashMap<u32, String>,
    pid: u32,
    version: String,
    ticks_per_sec: usize,
    pagesz: usize,
    now: u64,
}

impl MockSystem {
    pub fn new() -> MockSystemBuilder {
        MockSystemBuilder {
            ..Default::default()
        }
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

    fn user_by_uid(&self, uid: u32) -> Option<String> {
        match self.users.get(&uid) {
            Some(s) => Some(s.clone()),
            None => None,
        }
    }

    fn create_lock_file(&self, _p: &path::PathBuf) -> io::Result<fs::File> {
        panic!("Not in use yet");
    }

    fn remove_lock_file(&self, _p: path::PathBuf) -> io::Result<()> {
        panic!("Not in use yet");
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

    fn handle_interruptions(&self) {
        // Nothing yet
    }

    fn is_interrupted(&self) -> bool {
        // Nothing yet
        false
    }
}
