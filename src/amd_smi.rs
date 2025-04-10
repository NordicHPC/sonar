// Rust wrapper around ../gpuapi/sonar-amd.{c,h}.

use crate::gpuapi;
use crate::ps;
use crate::util::cstrdup;

////// C library API //////////////////////////////////////////////////////////////////////////////

// The data structures and signatures defined here must be exactly those defined in the header file,
// using types from `cty`.  See ../gpuapi/sonar-amd.h for all documentation of functionality and
// units.
//
// TODO: We should use bindgen for this but not important at the moment.

#[link(name = "sonar-amd", kind = "static")]
extern "C" {
    pub fn amdml_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

#[repr(C)]
pub struct AmdmlCardInfo {
    bus_addr: [cty::c_char; 80],
    model: [cty::c_char; 256],
    driver: [cty::c_char; 64],
    firmware: [cty::c_char; 32],
    uuid: [cty::c_char; 96],
    mem_total: cty::uint64_t,
    power_limit: cty::c_uint,
    min_power_limit: cty::c_uint,
    max_power_limit: cty::c_uint,
    min_ce_clock: cty::c_uint,
    max_ce_clock: cty::c_uint,
    min_mem_clock: cty::c_uint,
    max_mem_clock: cty::c_uint,
}

impl Default for AmdmlCardInfo {
    fn default() -> Self {
        Self {
            bus_addr: [0; 80],
            model: [0; 256],
            driver: [0; 64],
            firmware: [0; 32],
            uuid: [0; 96],
            mem_total: 0,
            power_limit: 0,
            min_power_limit: 0,
            max_power_limit: 0,
            min_ce_clock: 0,
            max_ce_clock: 0,
            min_mem_clock: 0,
            max_mem_clock: 0,
        }
    }
}

#[link(name = "sonar-amd", kind = "static")]
extern "C" {
    pub fn amdml_device_get_card_info(device: cty::uint32_t, buf: *mut AmdmlCardInfo)
        -> cty::c_int;
}

#[repr(C)]
#[derive(Default)]
pub struct AmdmlCardState {
    fan_speed_pct: cty::c_float,
    perf_level: cty::c_int,
    mem_used: cty::uint64_t,
    gpu_util: cty::c_float,
    mem_util: cty::c_float,
    temp: cty::c_uint,
    power: cty::c_uint,
    power_limit: cty::c_uint,
    ce_clock: cty::c_uint,
    mem_clock: cty::c_uint,
}

#[link(name = "sonar-amd", kind = "static")]
extern "C" {
    pub fn amdml_device_get_card_state(
        device: cty::uint32_t,
        buf: *mut AmdmlCardState,
    ) -> cty::c_int;
}

#[repr(C)]
#[derive(Default)]
pub struct AmdmlGpuProcess {
    pid: cty::uint32_t,
    cards: cty::uint32_t,
    gpu_util: cty::uint32_t,
    mem_util: cty::uint32_t,
    mem_size: cty::uint64_t,
}

#[link(name = "sonar-amd", kind = "static")]
extern "C" {
    pub fn amdml_device_probe_processes(count: *mut cty::uint32_t) -> cty::c_int;
    pub fn amdml_get_process(index: cty::uint32_t, buf: *mut AmdmlGpuProcess) -> cty::c_int;
    pub fn amdml_free_processes();
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn get_card_configuration() -> Option<Vec<gpuapi::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { amdml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: AmdmlCardInfo = Default::default();
    for dev in 0..num_devices {
        if unsafe { amdml_device_get_card_info(dev, &mut infobuf) } == 0 {
            let model = cstrdup(&infobuf.model);
            // This is a bit of a hack, really we'd prefer the underlying microarchitecture eg
            // TeraScale, GCN, RDNA, but grabbing the marketing name is the closest we get with
            // current SMI interfaces.  The marketing name is normally(?) in brackets in the model
            // name.
            let mut arch = "".to_string();
            if let Some((_, after)) = model.split_once("[Radeon") {
                if let Some((a, _)) = after.split_once("]") {
                    arch = "Radeon".to_string() + a;
                }
            }
            result.push(gpuapi::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpuapi::GpuName {
                    index: dev as i32,
                    uuid: cstrdup(&infobuf.uuid),
                },
                model: model,
                arch: arch,
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                mem_size_kib: (infobuf.mem_total / 1024) as i64,
                power_limit_watt: (infobuf.power_limit / 1000) as i32,
                max_power_limit_watt: (infobuf.max_power_limit / 1000) as i32,
                min_power_limit_watt: (infobuf.min_power_limit / 1000) as i32,
                max_ce_clock_mhz: infobuf.max_ce_clock as i32,
                max_mem_clock_mhz: infobuf.max_mem_clock as i32,
            })
        }
    }

    Some(result)
}

pub fn get_card_utilization() -> Option<Vec<gpuapi::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { amdml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: AmdmlCardState = Default::default();
    for dev in 0..num_devices {
        if unsafe { amdml_device_get_card_state(dev, &mut infobuf) } == 0 {
            result.push(gpuapi::CardState {
                device: gpuapi::GpuName {
                    index: dev as i32,
                    uuid: get_card_uuid(dev),
                },
                failing: 0,
                fan_speed_pct: infobuf.fan_speed_pct,
                compute_mode: "".to_string(),
                perf_state: infobuf.perf_level as i64,
                mem_reserved_kib: 0,
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
                device: gpuapi::GpuName {
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
    if unsafe { amdml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut infobuf: AmdmlGpuProcess = Default::default();
    let mut num_processes: cty::uint32_t = 0;
    if unsafe { amdml_device_probe_processes(&mut num_processes) } != 0 {
        return None;
    }

    for proc in 0..num_processes {
        if unsafe { amdml_get_process(proc, &mut infobuf) } != 0 {
            continue;
        }

        let (username, uid) = ptable.lookup(infobuf.pid as ps::Pid);
        let mut indices = infobuf.cards as usize;
        let mut k = 0u32;
        let mut devices = vec![];
        while indices != 0 {
            if (indices & 1) == 1 {
                devices.push(gpuapi::GpuName {
                    index: k as i32,
                    uuid: get_card_uuid(k),
                });
            }
            indices >>= 1;
            k += 1;
        }
        result.push(gpuapi::Process {
            devices,
            pid: infobuf.pid as usize,
            user: username,
            uid: uid,
            mem_pct: infobuf.mem_util as f64,
            gpu_pct: infobuf.gpu_util as f64,
            mem_size_kib: (infobuf.mem_size / 1024) as usize,
            command: None,
        })
    }

    unsafe { amdml_free_processes() };

    Some(result)
}

fn get_card_uuid(dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: AmdmlCardInfo = Default::default();
    if unsafe { amdml_device_get_card_info(dev, &mut infobuf) } == 0 {
        cstrdup(&infobuf.uuid)
    } else {
        "".to_string()
    }
}
