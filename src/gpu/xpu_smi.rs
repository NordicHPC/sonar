// Rust wrapper around ../../gpuapi/sonar-xpu.{c,h}.

use crate::gpu::{self, xpu::XpuGPU};
use crate::ps;
use crate::types::{Pid, Uid};
use crate::util::cstrdup;

use std::path::Path;

////// C library API //////////////////////////////////////////////////////////////////////////////

// The data structures and signatures defined here must be exactly those defined in the header file,
// using types from `cty`.  See ../../gpuapi/sonar-xpu.h for all documentation of functionality and
// units.
//
// TODO: We should use bindgen for this but not important at the moment.

#[link(name = "sonar-xpu", kind = "static")]
extern "C" {
    pub fn xpu_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

#[repr(C)]
pub struct XpuCardInfo {
    bus_addr: [cty::c_char; 256],
    model: [cty::c_char; 256],
    driver: [cty::c_char; 256],
    firmware: [cty::c_char; 256],
    uuid: [cty::c_char; 256],
    totalmem: cty::uint64_t,
    max_ce_clock: cty::c_uint,
    max_power_limit: cty::c_uint,
}

impl Default for XpuCardInfo {
    fn default() -> Self {
        Self {
            bus_addr: [0; 256],
            model: [0; 256],
            driver: [0; 256],
            firmware: [0; 256],
            uuid: [0; 256],
            totalmem: 0,
            max_ce_clock: 0,
            max_power_limit: 0,
        }
    }
}

#[link(name = "sonar-xpu", kind = "static")]
extern "C" {
    pub fn xpu_device_get_card_info(device: cty::uint32_t, buf: *mut XpuCardInfo) -> cty::c_int;
}

const PERF_STATE_UNKNOWN: cty::c_int = -1;

#[repr(C)]
#[derive(Default)]
pub struct XpuCardState {
    gpu_util: cty::c_float,
    mem_util: cty::c_float,
    mem_used: cty::uint64_t,
    temp: cty::c_uint,
    power: cty::c_uint,
    ce_clock: cty::c_uint,
}

#[link(name = "sonar-xpu", kind = "static")]
extern "C" {
    pub fn xpu_device_get_card_state(device: cty::uint32_t, buf: *mut XpuCardState) -> cty::c_int;
}

#[repr(C)]
#[derive(Default)]
pub struct XpuGpuProcess {
    pid: cty::uint32_t,
    mem_util: cty::uint32_t,
    gpu_util: cty::uint32_t,
    mem_size: cty::uint64_t,
}

#[link(name = "sonar-xpu", kind = "static")]
extern "C" {
    pub fn xpu_device_probe_processes(
        device: cty::uint32_t,
        count: *mut cty::uint32_t,
    ) -> cty::c_int;
    pub fn xpu_get_process(process_index: cty::uint32_t, buf: *mut XpuGpuProcess) -> cty::c_int;
    pub fn xpu_free_processes();
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn xpu_detect() -> bool {
    if Path::new("/sys/module/i915").exists() {
        let mut num_devices: cty::uint32_t = 0;
        unsafe { xpu_device_get_count(&mut num_devices) != -1 }
    } else {
        false
    }
}

pub fn get_card_configuration(xpu: &XpuGPU) -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { xpu_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: XpuCardInfo = Default::default();
    for dev in 0..num_devices {
        if unsafe { xpu_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(xpu, dev),
                },
                manufacturer: "Intel".to_string(),
                model: cstrdup(&infobuf.model),
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                arch: "Xpu".to_string(),
                mem_size_kib: (infobuf.totalmem / 1024),
                max_ce_clock_mhz: infobuf.max_ce_clock,
                max_power_limit_watt: infobuf.max_power_limit,
                max_mem_clock_mhz: 0,
                power_limit_watt: 0,
                min_power_limit_watt: 0,
            })
        }
    }

    Some(result)
}

pub fn get_card_utilization(xpu: &XpuGPU) -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { xpu_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: XpuCardState = Default::default();
    for dev in 0..num_devices {
        if unsafe { xpu_device_get_card_state(dev, &mut infobuf) } == 0 {
            result.push(gpu::CardState {
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(xpu, dev),
                },
                gpu_utilization_pct: infobuf.gpu_util,
                mem_utilization_pct: infobuf.mem_util,
                mem_used_kib: (infobuf.mem_used / 1024),
                temp_c: infobuf.temp,
                power_watt: (infobuf.power / 1000),
                ce_clock_mhz: infobuf.ce_clock,
                perf_state: PERF_STATE_UNKNOWN as i64,
                compute_mode: "".to_string(),
                fan_speed_pct: 0.0,
                failing: 0,
                mem_clock_mhz: 0,
                mem_reserved_kib: 0,
                power_limit_watt: 0,
            })
        } else {
            result.push(gpu::CardState {
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(xpu, dev),
                },
                failing: gpu::GENERIC_FAILURE,
                ..Default::default()
            })
        }
    }

    Some(result)
}

pub fn get_process_utilization(
    xpu: &XpuGPU,
    ptable: &ps::ProcessTable,
) -> Option<Vec<gpu::Process>> {
    let mut result = vec![];

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { xpu_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut infobuf: XpuGpuProcess = Default::default();
    for dev in 0..num_devices {
        let mut num_processes: cty::uint32_t = 0;
        if unsafe { xpu_device_probe_processes(dev, &mut num_processes) } != 0 {
            continue;
        }

        for proc in 0..num_processes {
            if unsafe { xpu_get_process(proc, &mut infobuf) } != 0 {
                continue;
            }

            let (username, uid) = ptable.lookup(infobuf.pid as Pid);
            result.push(gpu::Process {
                devices: vec![gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(xpu, dev),
                }],
                pid: infobuf.pid as Pid,
                user: username.clone(),
                uid: uid as Uid,
                mem_pct: infobuf.mem_util as f32,
                gpu_pct: infobuf.gpu_util as f32,
                mem_size_kib: infobuf.mem_size,
                command: None,
            })
        }

        unsafe { xpu_free_processes() };
    }

    Some(result)
}

fn get_card_uuid(xpu: &XpuGPU, dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: XpuCardInfo = Default::default();
    if unsafe { xpu_device_get_card_info(dev, &mut infobuf) } == 0 {
        #[cfg(debug_assertions)]
        let uuid = if std::env::var("SONARTEST_FAIL_UUID").is_ok() {
            "".to_string()
        } else {
            cstrdup(&infobuf.uuid)
        };
        #[cfg(not(debug_assertions))]
        let uuid = cstrdup(&infobuf.uuid);
        if uuid != "" {
            uuid
        } else {
            format!(
                "{}/{}/{}",
                &xpu.hostname,
                xpu.boot_time,
                cstrdup(&infobuf.bus_addr)
            )
        }
    } else {
        // Fall back to using the device number as the bus address
        format!("{}/{}/xpu#{dev}", &xpu.hostname, xpu.boot_time)
    }
}
