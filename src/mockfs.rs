// MockFS is used for testing, it is instantiated with the values we want it to return.

use crate::procfsapi;

use std::collections::HashMap;

pub struct MockFS {
    files: HashMap<String, String>,
    pids: Vec<(usize, u32)>,
}

impl MockFS {
    pub fn new(
        files: HashMap<String, String>,
        pids: Vec<(usize, u32)>,
    ) -> MockFS {
        MockFS {
            files,
            pids,
        }
    }
}

impl procfsapi::ProcfsAPI for MockFS {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        match self.files.get(path) {
            Some(s) => Ok(s.clone()),
            None => Err(format!("Unable to read /proc/{path}")),
        }
    }

    fn read_proc_pids(&self) -> Result<Vec<(usize, u32)>, String> {
        Ok(self.pids.clone())
    }
}
