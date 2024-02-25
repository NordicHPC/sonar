#[derive(PartialEq)]
pub struct Process {
    pub device: Option<usize>, // Device ID
    pub pid: usize,            // Process ID
    pub user: String,          // User name, _zombie_PID for zombies
    pub uid: usize,            // User ID, 666666 for zombies
    pub gpu_pct: f64,          // Percent of GPU /for this sample/, 0.0 for zombies
    pub mem_pct: f64,          // Percent of memory /for this sample/, 0.0 for zombies
    pub mem_size_kib: usize,   // Memory use in KiB /for this sample/, _not_ zero for zombies
    pub command: String,       // The command, _unknown_ for zombies, _noinfo_ if not known
}

pub const ZOMBIE_UID: usize = 666666;

#[derive(PartialEq)]
pub struct Card {
    pub model: String,
    pub mem_size_kib: i64,
}
