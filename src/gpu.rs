use crate::amd;
use crate::nvidia;
use crate::ps::UserTable;

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

#[derive(PartialEq, Default)]
pub struct Card {
    pub bus_addr: String,
    pub index: i32,       // Card index (changes at boot)
    pub model: String,    // NVIDIA: Product Name
    pub arch: String,     // NVIDIA: Product Architecture
    pub driver: String,   // NVIDIA: driver version
    pub firmware: String, // NVIDIA: CUDA version
    pub uuid: String,     // NVIDIA: The uuid
    pub mem_size_kib: i64,
    pub power_limit_watt: i32, // "current", but probably changes rarely
    pub max_power_limit_watt: i32,
    pub min_power_limit_watt: i32,
    pub max_ce_clock_mhz: i32,
    pub max_mem_clock_mhz: i32,
}

pub trait GPU {
    fn get_manufacturer(&self) -> String;
    fn get_configuration(&self) -> Result<Vec<Card>, String>;
    fn get_utilization(&self, user_by_pid: &UserTable) -> Result<Vec<Process>, String>;
}

pub fn probe() -> Option<Box<dyn GPU>> {
    if let Some(nvidia) = nvidia::probe() {
        Some(nvidia)
    } else if let Some(amd) = amd::probe() {
        Some(amd)
    } else {
        None
    }
}
