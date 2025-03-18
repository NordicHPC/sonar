// Low-level but common API to performance data for cards installed on the node.

use crate::ps::UserTable;

// The card index is zero-based and cards are densely packed in the index space.
//
// The uuid MUST NOT under any circumstances be confusable with some other device.  If a good uuid
// is not available from the card then it is acceptable for a card to be seen to have multiple
// (non-confusable) uuids over time.  It would be acceptable to construct one from, say,
// hostname:boot_time:bus_address (where hostname is as fully qualified as possible, ideally both
// cluster and node name).  Each GPU module (nvidia.rs, amd.rs, xpu.rs, etc) is responsible for
// managing the uuid.

#[derive(PartialEq, Eq, Hash, Default, Clone, Debug)]
pub struct GpuName {
    pub index: i32,             // May change at boot time
    pub uuid: String,           // Forever immutable
}

// Dynamic (per-sample) process information, across cards.  The GPU layer can report a single datum
// for a process across multiple cards (AMD, currently), or multiple data breaking down the process
// per card even if the process is running on multiple cards (NVIDIA, currently).  In the former
// case the computation could be wildly unbalanced but the Process datum will not reveal that by
// itself.  However by correlating CardState and Process something might be derived, especially if
// cards are not shared among processes.
//
// If the size of `devices` is larger than 1 then the values reported here should be divided among
// those devices, either evenly or (with CardState information in mind) in some kind of proportional
// manner.  The values of `gpu_pct`, `mem_pct` and `mem_size_kib` are the sums across all the
// `devices`.  Thus for four devices, `gpu_pct` can be up to 400.

#[derive(PartialEq, Default, Clone, Debug)]
pub struct Process {
    pub devices: Vec<GpuName>,   // Names are distinct
    pub pid: usize,              // Process ID
    pub user: String,            // User name, _zombie_PID for zombies
    pub uid: usize,              // User ID, 666666 for zombies
    pub gpu_pct: f64,            // Percent of GPU /for this sample/, 0.0 for zombies
    pub mem_pct: f64,            // Percent of memory /for this sample/, 0.0 for zombies
    pub mem_size_kib: usize,     // Memory use in KiB /for this sample/, _not_ zero for zombies
    pub command: Option<String>, // The command, _unknown_ for zombies, _noinfo_ if not known, None
                                 //   when the GPU layer simply can't know.
}

// Static (sample-invariant) card information.  The power limit is not static but in practice
// changes only very rarely.

#[derive(PartialEq, Default, Clone, Debug)]
pub struct Card {
    pub device: GpuName,
    pub bus_addr: String,
    pub model: String,    // NVIDIA: Product Name
    pub arch: String,     // NVIDIA: Product Architecture
    pub driver: String,   // NVIDIA: driver version
    pub firmware: String, // NVIDIA: CUDA version
    pub mem_size_kib: i64,
    pub power_limit_watt: i32, // "current", but probably changes rarely
    pub max_power_limit_watt: i32,
    pub min_power_limit_watt: i32,
    pub max_ce_clock_mhz: i32,
    pub max_mem_clock_mhz: i32,
}

// Dynamic (per-sample) card information, across processes
//
// If the card is OK then `failing` is 0, otherwise some error code listed below.

#[derive(PartialEq, Default, Clone, Debug)]
pub struct CardState {
    pub device: GpuName,
    pub failing: i32,
    pub fan_speed_pct: f32,
    pub compute_mode: String,
    pub perf_state: i64,
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

#[allow(dead_code)]
pub const GENERIC_FAILURE: i32 = 1;

// Trait representing the set of cards installed on a node.
pub trait GPU {
    // Retrieve the standard name of the manufacturer of the GPUs.  As get_manufacturer() is for the
    // GPU object as a whole and not per-card, we are currently assuming that nodes don't have cards
    // from multiple manufacturers.
    //
    // Names, once defined, will never change.  Current names: "NVIDIA", "AMD".
    fn get_manufacturer(&self) -> String;

    // Get static (or nearly static) information about the installed cards.
    //
    // The returned vector is sorted by the card's name.index field, and card indices are tightly
    // packed in the array starting at zero.
    fn get_card_configuration(&self) -> Result<Vec<Card>, String>;

    // Get dynamic per-process information about jobs running on the installed cards.  See comment
    // at `Process`, above, for more about the meaning of these data.
    //
    // The returned vector is unsorted.
    fn get_process_utilization(
        &self,
        user_by_pid: &UserTable,
    ) -> Result<Vec<Process>, String>;

    // Get dynamic per-card information about the installed cards.
    //
    // The returned vector is sorted by the card's name.index field, and card indices are tightly
    // packed in the array starting at zero.
    fn get_card_utilization(&self) -> Result<Vec<CardState>, String>;
}

// Probe the node for installed cards and return an object representing them, if any are found.
pub trait GpuAPI {
    fn probe(&self) -> Option<Box<dyn GPU>>;
}
