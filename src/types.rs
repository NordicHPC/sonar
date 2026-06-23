// Some basic data types so that we can avoid tying ourselves to an integer type.

use std::fmt;

// 32-bit definitions are OK for testing.
//
// pub type JobID = u32;
// pub type Pid = u32;
// pub type Uid = u32;
//
// pub const PID_MAX: Pid = u32::MAX;

pub type JobID = u64;
pub type Uid = u64;

#[derive(PartialEq,Eq,Clone,Copy,Debug,Hash)]
pub struct Pid {
    p: u64,
}

impl Pid {
    pub fn new(p: u64) -> Pid {
        assert!(p != 0);
        Pid{p}
    }

    pub fn get(&self) -> u64 {
        self.p
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.p);
        Ok(())
    }
}

#[allow(dead_code)]
pub const PID_MAX: Pid = u64::MAX;
