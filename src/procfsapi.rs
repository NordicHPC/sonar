// This creates a API by which procfs can access the underlying computing system, allowing the
// system to be virtualized.  In turn, that allows sensible test cases to be written.

extern crate libc;
extern crate page_size;

use crate::users::get_user_by_uid;

use std::fs;
use std::os::linux::fs::MetadataExt;
use std::path;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::collections::HashMap;

pub trait ProcfsAPI {
    // Open /proc/<path> (which can have multiple path elements, eg, {PID}/filename), read it, and
    // return its entire contents as a string.  Return a sensible error message if the file can't
    // be opened or read.
    fn read_to_string(&self, path: &str) -> Result<String, String>;

    // Return (pid,uid) for every file /proc/{PID}.  Return a sensible error message in case
    // something goes really, really wrong, but otherwise try to make the best of it.
    fn read_proc_pids(&self) -> Result<Vec<(usize, u32)>, String>;

    // Try to figure out the user's name from system tables, this may be an expensive operation.
    fn user_by_uid(&self, uid: u32) -> Option<String>;

    // Return the value of CLK_TCK, or 0 on error.
    fn clock_ticks_per_sec(&self) -> usize;

    // Return the page size measured in KB
    fn page_size_in_kib(&self) -> usize;

    // Return the current time in seconds since Unix epoch.
    fn now_in_secs_since_epoch(&self) -> u64;
}

// RealFS is used to actually access /proc, system tables, and system clock.

pub struct RealFS {}

impl RealFS {
    pub fn new() -> RealFS {
        RealFS {}
    }
}

impl ProcfsAPI for RealFS {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        let filename = format!("/proc/{path}");
        match fs::read_to_string(path::Path::new(&filename)) {
            Ok(s) => Ok(s),
            Err(_) => Err(format!("Unable to read {filename}")),
        }
    }

    fn read_proc_pids(&self) -> Result<Vec<(usize, u32)>, String> {
        let mut pids = vec![];
        if let Ok(dir) = fs::read_dir("/proc") {
            for dirent in dir {
                if let Ok(dirent) = dirent {
                    if let Ok(meta) = dirent.metadata() {
                        let uid = meta.st_uid();
                        if let Some(name) = dirent.path().file_name() {
                            if let Ok(pid) = name.to_string_lossy().parse::<usize>() {
                                pids.push((pid, uid));
                            }
                        }
                    }
                }
            }
        } else {
            return Err("Could not open /proc".to_string());
        };
        Ok(pids)
    }

    fn user_by_uid(&self, uid: u32) -> Option<String> {
        get_user_by_uid(uid).map(|u| u.to_string_lossy().to_string())
    }

    fn clock_ticks_per_sec(&self) -> usize {
        unsafe { libc::sysconf(libc::_SC_CLK_TCK) as usize }
    }

    fn page_size_in_kib(&self) -> usize {
        page_size::get() / 1024
    }

    fn now_in_secs_since_epoch(&self) -> u64 {
        unix_now()
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn parse_usize_field(
    fields: &[&str],
    ix: usize,
    line: &str,
    file: &str,
    pid: usize,
    fieldname: &str,
) -> Result<usize, String> {
    if ix >= fields.len() {
        if pid == 0 {
            return Err(format!("Index out of range for /proc/{file}: {ix}: {line}"));
        } else {
            return Err(format!(
                "Index out of range for /proc/{pid}/{file}: {ix}: {line}"
            ));
        }
    }
    if let Ok(n) = fields[ix].parse::<usize>() {
        return Ok(n);
    }
    if pid == 0 {
        Err(format!(
            "Could not parse {fieldname} in /proc/{file}: {line}"
        ))
    } else {
        Err(format!(
            "Could not parse {fieldname} from /proc/{pid}/{file}: {line}"
        ))
    }
}

// MockFS is used for testing, it is instantiated with the values we want it to return.

#[cfg(test)]
pub struct MockFS {
    files: HashMap<String, String>,
    pids: Vec<(usize, u32)>,
    users: HashMap<u32, String>,
    ticks_per_sec: usize,
    pagesz: usize,
    now: u64,
}

#[cfg(test)]
impl MockFS {
    pub fn new(
        files: HashMap<String, String>,
        pids: Vec<(usize, u32)>,
        users: HashMap<u32, String>,
        now: u64,
    ) -> MockFS {
        MockFS {
            files,
            pids,
            users,
            ticks_per_sec: 100,
            pagesz: 4,
            now,
        }
    }
}

#[cfg(test)]
impl ProcfsAPI for MockFS {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        match self.files.get(path) {
            Some(s) => Ok(s.clone()),
            None => Err(format!("Unable to read /proc/{path}")),
        }
    }

    fn read_proc_pids(&self) -> Result<Vec<(usize, u32)>, String> {
        Ok(self.pids.clone())
    }

    fn user_by_uid(&self, uid: u32) -> Option<String> {
        match self.users.get(&uid) {
            Some(s) => Some(s.clone()),
            None => None,
        }
    }

    fn clock_ticks_per_sec(&self) -> usize {
        self.ticks_per_sec
    }

    fn page_size_in_kib(&self) -> usize {
        self.pagesz
    }

    fn now_in_secs_since_epoch(&self) -> u64 {
        self.now
    }
}
