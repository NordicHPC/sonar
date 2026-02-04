// Some basic data types so that we can avoid tying ourselves to an integer type.

// 32-bit definitions are OK for testing.
//
// pub type JobID = u32;
// pub type Pid = u32;
// pub type Uid = u32;
//
// pub const PID_MAX: Pid = u32::MAX;

pub type JobID = u64;
pub type Pid = u64;
pub type Uid = u64;

#[allow(dead_code)]
pub const PID_MAX: Pid = u64::MAX;
