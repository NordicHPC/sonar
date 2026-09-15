// Some basic data types so that we can avoid tying ourselves to an integer type.

// 32-bit definitions are OK for testing.
//
// pub type JobID = u32;
// pub type Pid = u32;
// pub type Uid = u32;
//
// pub const PID_MAX: Pid = u32::MAX;

pub type JobID = u64;
//pub type Pid = u64;
pub type Uid = u64;

#[derive(Clone,Copy,PartialEq,Eq,Debug,Hash)]
pub struct Pid {
    pub p: u64,
}

impl Pid {
    pub fn new(p: u64) -> Pid {
        assert!(p != 0 && p != u64::MAX);
        Pid{ p }
    }

    pub fn maybe(p: u64) -> Option<Pid> {
        if p == 0 {
            None
        } else {
            assert!(p != u64::MAX);
            Some(Pid{ p })
        }
    }

    pub fn max() -> Pid {
        Pid{ p: u64::MAX }
    }
}

impl std::fmt::Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.p)
    }
}
