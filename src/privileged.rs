// Code to talk to the sonar runner that runs as root and performs limited lookups of privileged
// information on our behalf.
//
// The protocol is defined in C code in ../runner/.
//
// Note that if the server is hung then these functions will hang too.  It may be that a watchdog
// timer is necessary to fix this: if the timer triggers, the server should be considered dead.  But
// this is probably too much.  For now, assume the server is reliable and will crash itself (and
// Sonar) if it gets into an error state.

use std::cell;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::io::FromRawFd;

#[allow(dead_code)]
const REQ_EXIT: u8 = 0u8;
const REQ_EXE_FOR_PIDS: u8 = 1u8;

// Given a real PID, read the /proc/PID/exe link.
//
// OPTIMIZEME: What we really want to do here is to send all the requests for all the PIDs at the
// same time, less overhead even if it means managing dynamic buffers.

pub fn get_exe_path(pid: u32, srv: &RootServer) -> Result<String, String> {
    let mut req = Outgoing::new();
    req.put_u8(REQ_EXE_FOR_PIDS);
    req.put_u32(1u32); // 1 pid
    req.put_u32(pid); // The pid
    srv.send(req)?;

    let mut resp = srv.receive()?;
    if resp.get_u8()? != REQ_EXE_FOR_PIDS {
        return Err("Bad op".to_string());
    }
    if resp.get_u32()? != 1 {
        return Err("Bad array length".to_string());
    }
    let _pid = resp.get_u32()?;
    Ok(resp.get_string()?)
}

// Protocol logic implementation.

pub struct Outgoing {
    v: Vec<u8>,
}

impl Outgoing {
    pub fn new() -> Outgoing {
        let mut v = Vec::<u8>::with_capacity(20); // 20 is plenty for short messages
        v.resize(4, 0u8); // Header used by RootServer
        Outgoing { v }
    }

    pub fn put_u8(&mut self, n: u8) {
        self.v.push(n);
    }

    pub fn put_u32(&mut self, mut n: u32) {
        self.v.push((n & 255) as u8);
        n >>= 8;
        self.v.push((n & 255) as u8);
        n >>= 8;
        self.v.push((n & 255) as u8);
        n >>= 8;
        self.v.push((n & 255) as u8);
    }

    // Missing because not needed: put_string
}

pub struct Incoming {
    v: Vec<u8>,
    ix: usize,
}

impl Incoming {
    pub fn new(size: usize) -> Incoming {
        let mut v = Vec::<u8>::with_capacity(size);
        v.resize(size, 0u8);
        Incoming { v: v, ix: 0 }
    }

    pub fn get_u8(&mut self) -> Result<u8, String> {
        if self.ix + 1 <= self.v.len() {
            let b = self.v[self.ix];
            self.ix += 1;
            Ok(b)
        } else {
            Err("Out of bounds".to_string())
        }
    }

    pub fn get_u32(&mut self) -> Result<u32, String> {
        if self.ix + 4 <= self.v.len() {
            let mut n = self.v[self.ix] as u32;
            n |= (self.v[self.ix + 1] as u32) << 8;
            n |= (self.v[self.ix + 2] as u32) << 16;
            n |= (self.v[self.ix + 3] as u32) << 24;
            self.ix += 4;
            Ok(n)
        } else {
            Err("Out of bounds".to_string())
        }
    }

    pub fn get_string(&mut self) -> Result<String, String> {
        let l = self.get_u32()? as usize;
        if self.ix + l <= self.v.len() {
            let s = String::from_utf8_lossy(&self.v[self.ix..self.ix + l]).to_string();
            self.ix += l;
            Ok(s)
        } else {
            Err("Out of bounds".to_string())
        }
    }
}

pub struct RootServer {
    request: cell::RefCell<fs::File>,
    response: cell::RefCell<fs::File>,
}

impl RootServer {
    pub fn new(request_fd: u32, response_fd: u32) -> RootServer {
        // Per the docs, from_raw_fd is unsafe because it may be unsound to make more than one File
        // from any given descriptor.
        RootServer {
            request: cell::RefCell::new(unsafe { fs::File::from_raw_fd(request_fd as i32) }),
            response: cell::RefCell::new(unsafe { fs::File::from_raw_fd(response_fd as i32) }),
        }
    }

    pub fn send(&self, mut msg: Outgoing) -> Result<(), String> {
        let l = msg.v.len() as u32 - 4;
        msg.v[0] = (l & 255) as u8;
        msg.v[1] = ((l >> 8) & 255) as u8;
        msg.v[2] = ((l >> 16) & 255) as u8;
        msg.v[3] = ((l >> 24) & 255) as u8;
        self.write(&msg.v.as_slice())
    }

    pub fn receive(&self) -> Result<Incoming, String> {
        let mut szbuf = [0u8; 4];
        self.read(&mut szbuf)?;
        let mut size = szbuf[0] as usize;
        size |= (szbuf[1] as usize) << 8;
        size |= (szbuf[2] as usize) << 16;
        size |= (szbuf[3] as usize) << 32;
        let mut incoming = Incoming::new(size);
        self.read(incoming.v.as_mut_slice())?;
        Ok(incoming)
    }

    fn write(&self, bytes: &[u8]) -> Result<(), String> {
        match self.request.borrow_mut().write_all(bytes) {
            Ok(_) => Ok(()),
            Err(_) => Err("Could not write".to_string()),
        }
    }

    fn read(&self, bytes: &mut [u8]) -> Result<(), String> {
        match self.response.borrow_mut().read_exact(bytes) {
            Ok(_) => Ok(()),
            Err(_) => Err("Could not read".to_string()),
        }
    }
}
