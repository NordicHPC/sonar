// Rust wrapper around ../gpuapi/sonar-nvidia.{c,h}.

use crate::gpuapi;
use crate::ps;
use crate::util::cstrdup;

////// C library API //////////////////////////////////////////////////////////////////////////////

// The data structures and signatures defined here must be exactly those defined in the header file,
// using types from `cty`.  See ../gpuapi/sonar-nvidia.h for all documentation of functionality and
// units.
//
// TODO: We should use bindgen for this but not important at the moment.

#[link(name = "sonar-nvidia", kind = "static")]
extern "C" {
    pub fn nvml_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

#[repr(C)]
pub struct NvmlCardInfo {
    bus_addr: [cty::c_char; 80],
    model: [cty::c_char; 96],
    architecture: [cty::c_char; 32],
    driver: [cty::c_char; 80],
    firmware: [cty::c_char; 32],
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
            firmware: [0; 32],
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

#[link(name = "sonar-nvidia", kind = "static")]
extern "C" {
    pub fn nvml_device_get_card_info(device: cty::uint32_t, buf: *mut NvmlCardInfo) -> cty::c_int;
}

const COMP_MODE_UNKNOWN: cty::c_int = -1;
const COMP_MODE_DEFAULT: cty::c_int = 0;
const COMP_MODE_PROHIBITED: cty::c_int = 1;
const COMP_MODE_EXCLUSIVE_PROCESS: cty::c_int = 2;

const PERF_STATE_UNKNOWN: cty::c_int = -1;

#[repr(C)]
#[derive(Default)]
pub struct NvmlCardState {
    fan_speed: cty::c_uint,
    compute_mode: cty::c_int,
    perf_state: cty::c_int,
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

#[link(name = "sonar-nvidia", kind = "static")]
extern "C" {
    pub fn nvml_device_get_card_state(device: cty::uint32_t, buf: *mut NvmlCardState)
        -> cty::c_int;
}

#[repr(C)]
#[derive(Default)]
pub struct NvmlGpuProcess {
    pid: cty::uint32_t,
    mem_util: cty::uint32_t,
    gpu_util: cty::uint32_t,
    mem_size: cty::uint64_t,
}

#[link(name = "sonar-nvidia", kind = "static")]
extern "C" {
    pub fn nvml_device_probe_processes(
        device: cty::uint32_t,
        count: *mut cty::uint32_t,
    ) -> cty::c_int;
    pub fn nvml_get_process(index: cty::uint32_t, buf: *mut NvmlGpuProcess) -> cty::c_int;
    pub fn nvml_free_processes();
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn get_card_configuration() -> Option<Vec<gpuapi::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: NvmlCardInfo = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvml_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpuapi::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpuapi::GpuName{
                    index: dev as i32,
                    uuid: cstrdup(&infobuf.uuid),
                },
                model: cstrdup(&infobuf.model),
                arch: cstrdup(&infobuf.architecture),
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                mem_size_kib: (infobuf.mem_total / 1024) as i64,
                power_limit_watt: (infobuf.power_limit / 1000) as i32,
                max_power_limit_watt: (infobuf.min_power_limit / 1000) as i32,
                min_power_limit_watt: (infobuf.max_power_limit / 1000) as i32,
                max_ce_clock_mhz: infobuf.max_ce_clock as i32,
                max_mem_clock_mhz: infobuf.max_mem_clock as i32,
            })
        }
    }

    Some(result)
}

pub fn get_card_utilization() -> Option<Vec<gpuapi::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: NvmlCardState = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvml_device_get_card_state(dev, &mut infobuf) } == 0 {
            let mode = match infobuf.compute_mode {
                COMP_MODE_DEFAULT => "",
                COMP_MODE_PROHIBITED => "Prohibited",
                COMP_MODE_EXCLUSIVE_PROCESS => "ExclusiveProcess",
                COMP_MODE_UNKNOWN | _ => "Unknown",
            };
            let perf = match infobuf.perf_state {
                PERF_STATE_UNKNOWN => -1,
                x => x,
            };
            result.push(gpuapi::CardState {
                device: gpuapi::GpuName{
                    index: dev as i32,
                    uuid: get_card_uuid(dev),
                },
                failing: 0,
                fan_speed_pct: infobuf.fan_speed as f32,
                compute_mode: mode.to_string(),
                perf_state: perf as i64,
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
        } else {
            result.push(gpuapi::CardState {
                device: gpuapi::GpuName{
                    index: dev as i32,
                    uuid: get_card_uuid(dev),
                },
                failing: gpuapi::GENERIC_FAILURE,
                ..Default::default()
            })
        }
    }

    Some(result)
}

pub fn get_process_utilization(ptable: &ps::ProcessTable) -> Option<Vec<gpuapi::Process>> {
    let mut result = vec![];

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut infobuf: NvmlGpuProcess = Default::default();
    for dev in 0..num_devices {
        let mut num_processes: cty::uint32_t = 0;
        if unsafe { nvml_device_probe_processes(dev, &mut num_processes) } != 0 {
            continue;
        }

        for proc in 0..num_processes {
            if unsafe { nvml_get_process(proc, &mut infobuf) } != 0 {
                continue;
            }

            let (username, uid) = ptable.lookup(infobuf.pid as ps::Pid);
            result.push(gpuapi::Process {
                devices: vec![gpuapi::GpuName{
                    index: dev as i32,
                    uuid: get_card_uuid(dev),
                }],
                pid: infobuf.pid as usize,
                user: username.to_string(),
                uid: uid,
                mem_pct: infobuf.mem_util as f64,
                gpu_pct: infobuf.gpu_util as f64,
                mem_size_kib: infobuf.mem_size as usize,
                command: None,
            })
        }

        unsafe { nvml_free_processes() };
    }

    Some(result)
}

pub fn get_card_uuid(dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: NvmlCardInfo = Default::default();
    if unsafe { nvml_device_get_card_info(dev, &mut infobuf) } == 0 {
        cstrdup(&infobuf.uuid)
    } else {
        "".to_string()
    }
}
