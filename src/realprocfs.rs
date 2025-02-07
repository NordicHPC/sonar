use crate::procfsapi;

use std::fs;
use std::os::linux::fs::MetadataExt;
use std::path;

// RealProcFS is used to actually access /proc, system tables, and system clock.

pub struct RealProcFS {}

impl RealProcFS {
    pub fn new() -> RealProcFS {
        RealProcFS {}
    }
}

impl procfsapi::ProcfsAPI for RealProcFS {
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
            for dirent in dir.flatten() {
                if let Ok(meta) = dirent.metadata() {
                    let uid = meta.st_uid();
                    if let Some(name) = dirent.path().file_name() {
                        if let Ok(pid) = name.to_string_lossy().parse::<usize>() {
                            pids.push((pid, uid));
                        }
                    }
                }
            }
        } else {
            return Err("Could not open /proc".to_string());
        };
        Ok(pids)
    }
}
