#[cfg(feature = "amd")]
use crate::amd;
use crate::gpuset;
#[cfg(feature = "nvidia")]
use crate::nvidia;
#[cfg(feature = "xpu")]
use crate::xpu;
use crate::ps;

// Per-sample process information, across cards.  The GPU layer can report a single datum for a
// process across multiple cards, or multiple data breaking down the process per card even if the
// process is running on multiple cards.

#[derive(PartialEq, Default, Clone, Debug)]
pub struct Process {
    pub devices: gpuset::GpuSet, // Device IDs
    pub pid: usize,              // Process ID
    pub user: String,            // User name, _zombie_PID for zombies
    pub uid: usize,              // User ID, 666666 for zombies
    pub gpu_pct: f64,            // Percent of GPU /for this sample/, 0.0 for zombies
    pub mem_pct: f64,            // Percent of memory /for this sample/, 0.0 for zombies
    pub mem_size_kib: usize,     // Memory use in KiB /for this sample/, _not_ zero for zombies
    pub command: Option<String>, // The command, _unknown_ for zombies, _noinfo_ if not known, None
                                 //   when the GPU layer simply can't know.
}

// Sample-invariant card information

#[derive(PartialEq, Default, Clone, Debug)]
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

// Per-sample card information, across processes

#[derive(PartialEq, Default, Clone, Debug)]
pub struct CardState {
    pub index: i32, // Stable card identifier
    pub fan_speed_pct: f32,
    pub compute_mode: String,
    pub perf_state: String,
    pub mem_reserved_kib: i64,
    pub mem_used_kib: i64,
    pub gpu_utilization_pct: f32,
    pub mem_utilization_pct: f32,
    pub temp_c: i32,
    pub power_watt: i32,
    pub power_limit_watt: i32,
    pub ce_clock_mhz: i32,
    pub mem_clock_mhz: i32,
}

// Abstract GPU information across GPU types.
//
// As get_manufacturer() is for the GPU object as a whole and not per-card, we are currently
// assuming that nodes don't have cards from multiple manufacturers.
//
// get_card_configuration() and get_card_utilization() return vectors that are sorted by their index
// fields, and indices shall be tightly packed.

pub trait GPU {
    fn get_manufacturer(&mut self) -> String;
    fn get_card_configuration(&mut self) -> Result<Vec<Card>, String>;
    fn get_process_utilization(
        &mut self,
        user_by_pid: &ps::UserTable,
    ) -> Result<Vec<Process>, String>;
    fn get_card_utilization(&mut self) -> Result<Vec<CardState>, String>;
}

pub trait GpuAPI {
    fn probe(&self) -> Option<Box<dyn GPU>>;
}

pub struct RealGpuAPI {}

impl RealGpuAPI {
    pub fn new() -> RealGpuAPI {
        RealGpuAPI {}
    }
}

impl GpuAPI for RealGpuAPI {
    fn probe(&self) -> Option<Box<dyn GPU>> {
        #[cfg(feature = "nvidia")]
        if let Some(nvidia) = nvidia::probe() {
            return Some(nvidia);
        }
        #[cfg(feature = "amd")]
        if let Some(amd) = amd::probe() {
            return Some(amd)
        }
        #[cfg(feature = "xpu")]
        if let Some(xpu) = xpu::probe() {
            return Some(xpu)
        }
        return None
    }
}

#[cfg(test)]
pub struct MockGpuAPI {}

#[cfg(test)]
impl MockGpuAPI {
    pub fn new() -> MockGpuAPI {
        MockGpuAPI {}
    }
}

#[cfg(test)]
impl GpuAPI for MockGpuAPI {
    fn probe(&self) -> Option<Box<dyn GPU>> {
        None
    }
}
