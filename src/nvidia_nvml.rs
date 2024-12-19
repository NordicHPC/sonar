use crate::gpu;
use crate::ps::UserTable;
use std::ffi::CStr;

////// C library API //////////////////////////////////////////////////////////////////////////////

// These APIs must match the C APIs *exactly*.  For documentation of the units, see sonar-nvml.h.

// Should use bindgen for this but not important yet.

extern "C" {
    pub fn nvml_open() -> cty::c_int;
    pub fn nvml_close() -> cty::c_int;
    pub fn nvml_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

#[repr(C)]
pub struct NvmlCardInfo {
    bus_addr: [cty::c_char; 80],
    model: [cty::c_char; 96],
    architecture: [cty::c_char; 32],
    driver: [cty::c_char; 80],
    firmware: [cty::c_char; 80],
    uuid: [cty::c_char; 96],
    mem_total: cty::uint64_t,
    power_limit: cty::c_uint,
    min_power_limit: cty::c_uint,
    max_power_limit: cty::c_uint,
    max_ce_clock: cty::c_uint,
    max_mem_clock: cty::c_uint,
}

impl Default for NvmlCardInfo {
    fn default() -> Self {
        Self {
            bus_addr: [0; 80],
            model: [0; 96],
            architecture: [0; 32],
            driver: [0; 80],
            firmware: [0; 80],
            uuid: [0; 96],
            mem_total: 0,
            power_limit: 0,
            min_power_limit: 0,
            max_power_limit: 0,
            max_ce_clock: 0,
            max_mem_clock: 0,
        }
    }
}

extern "C" {
    pub fn nvml_device_get_card_info(device: cty::uint32_t, buf: *mut NvmlCardInfo) -> cty::c_int;
}

#[repr(C)]
pub struct NvmlCardState {
    fan_speed: cty::c_uint,
    compute_mode: [cty::c_char; 32],
    perf_state: [cty::c_char; 8],
    mem_reserved: cty::uint64_t,
    mem_used: cty::uint64_t,
    gpu_util: cty::c_float,
    mem_util: cty::c_float,
    temp: cty::c_uint,
    power: cty::c_uint,
    power_limit: cty::c_uint,
    ce_clock: cty::c_uint,
    mem_clock: cty::c_uint,
}

impl Default for NvmlCardState {
    fn default() -> Self {
        Self {
            fan_speed: 0,
            compute_mode: [0; 32],
            perf_state: [0; 8],
            mem_reserved: 0,
            mem_used: 0,
            gpu_util: 0.0,
            mem_util: 0.0,
            temp: 0,
            power: 0,
            power_limit: 0,
            ce_clock: 0,
            mem_clock: 0,
        }
    }
}

extern "C" {
    pub fn nvml_device_get_card_state(device: cty::uint32_t, buf: *mut NvmlCardState)
        -> cty::c_int;
}

////// End C library API //////////////////////////////////////////////////////

pub fn get_card_configuration() -> Option<Vec<gpu::Card>> {
    if unsafe { nvml_open() } != 0 {
        return None;
    }

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        unsafe { nvml_close() };
        return None;
    }

    let mut result = vec![];
    let mut infobuf: NvmlCardInfo = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvml_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                index: dev as i32,
                model: cstrdup(&infobuf.model),
                arch: cstrdup(&infobuf.architecture),
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                uuid: cstrdup(&infobuf.uuid),
                mem_size_kib: (infobuf.mem_total / 1024) as i64,
                power_limit_watt: (infobuf.power_limit / 1000) as i32,
                max_power_limit_watt: (infobuf.min_power_limit / 1000) as i32,
                min_power_limit_watt: (infobuf.max_power_limit / 1000) as i32,
                max_ce_clock_mhz: infobuf.max_ce_clock as i32,
                max_mem_clock_mhz: infobuf.max_mem_clock as i32,
            })
        }
    }

    unsafe { nvml_close() };
    Some(result)
}

// The requirement here is that we should also see orphaned processes.
//
// In terms of the nvml API:
//
//  - nvmlDeviceGetProcessUtilization() is like pmon and can get per-pid utilization
//  - nvmlDeviceGetComputeRunningProcesses_v3() will return a vector
//    of running processes, with pid and used memories.
//
// It's unclear if these two together are sufficient to get information about orphaned
// processes but it's a start.
//
// Possibly nvmlDeviceGetProcessesUtilizationInfo() is really the better API?
//
// MIG: Of the three, only nvmlDeviceGetComputeRunningProcesses_v3() is supported on MIG-enabled
// GPUs, and here information about other users' processes may not be available to unprivileged
// users.
//
// In either case, this will probably have some kind of setup / lookup / cleanup API,
// so that any memory management can be confined to the GPU layer.
//
// Not yet clear how to discover whether a node / card is in MIG mode.

pub fn get_process_utilization(_user_by_pid: &UserTable) -> Option<Vec<gpu::Process>> {
    // FIXME
    None
}

pub fn get_card_utilization() -> Option<Vec<gpu::CardState>> {
    if unsafe { nvml_open() } != 0 {
        return None;
    }

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        unsafe { nvml_close() };
        return None;
    }

    let mut result = vec![];
    let mut infobuf: NvmlCardState = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvml_device_get_card_state(dev, &mut infobuf) } == 0 {
            result.push(gpu::CardState {
                index: dev as i32,
                fan_speed_pct: infobuf.fan_speed as f32,
                compute_mode: cstrdup(&infobuf.compute_mode),
                perf_state: cstrdup(&infobuf.perf_state),
                mem_reserved_kib: (infobuf.mem_reserved / 1024) as i64,
                mem_used_kib: (infobuf.mem_used / 1024) as i64,
                gpu_utilization_pct: infobuf.gpu_util,
                mem_utilization_pct: infobuf.mem_util,
                temp_c: infobuf.temp as i32,
                power_watt: (infobuf.power / 1000) as i32,
                power_limit_watt: (infobuf.power_limit / 1000) as i32,
                ce_clock_mhz: infobuf.ce_clock as i32,
                mem_clock_mhz: infobuf.mem_clock as i32,
            })
        }
    }

    unsafe { nvml_close() };
    Some(result)
}

////////////////////////////////////////////////////////////////////////////////

// Utilities

// TODO: Share this with the code in time.rs.
fn cstrdup(s: &[i8]) -> String {
    unsafe { CStr::from_ptr(s.as_ptr()) }
        .to_str()
        .expect("Will always be utf8")
        .to_string()
}
