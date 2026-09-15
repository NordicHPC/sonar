use crate::gpu::{self, xpu_smi};
use crate::ps;
use crate::types::{Pid, Uid};
use crate::util::cstrdup::cstrdup;

use std::path::Path;

#[link(name = "sonar-xpu", kind = "static")]
unsafe extern "C" {}

pub struct XpuGPU {
    pub hostname: String,
    pub boot_time: u64,
}

pub fn probe(hostname: &str, boot_time: u64) -> Option<Box<dyn gpu::Gpu>> {
    if xpu_present() {
        Some(Box::new(XpuGPU {
            hostname: hostname.to_string(),
            boot_time,
        }))
    } else {
        None
    }
}

impl gpu::Gpu for XpuGPU {
    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = get_card_configuration(self) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &self,
        ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = get_process_utilization(self, ptable) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        if let Some(info) = get_card_utilization(self) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }
}

fn xpu_present() -> bool {
    xpu_detect()
}

// C interface

const PERF_STATE_UNKNOWN: cty::c_int = -1;

fn xpu_detect() -> bool {
    if Path::new("/sys/module/i915").exists() {
        let mut num_devices: cty::uint32_t = 0;
        unsafe { xpu_smi::xpu_device_get_count(&mut num_devices) != -1 }
    } else {
        false
    }
}

fn get_card_configuration(xpu: &XpuGPU) -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { xpu_smi::xpu_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: xpu_smi::xpu_card_info_t = Default::default();
    for dev in 0..num_devices {
        if unsafe { xpu_smi::xpu_device_get_card_info(dev, &mut infobuf) } == 0 {
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

fn get_card_utilization(xpu: &XpuGPU) -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { xpu_smi::xpu_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: xpu_smi::xpu_card_state_t = Default::default();
    for dev in 0..num_devices {
        if unsafe { xpu_smi::xpu_device_get_card_state(dev, &mut infobuf) } == 0 {
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

fn get_process_utilization(xpu: &XpuGPU, ptable: &ps::ProcessTable) -> Option<Vec<gpu::Process>> {
    let mut result = vec![];

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { xpu_smi::xpu_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut infobuf: xpu_smi::xpu_gpu_process_t = Default::default();
    for dev in 0..num_devices {
        let mut num_processes: cty::uint32_t = 0;
        if unsafe { xpu_smi::xpu_device_probe_processes(dev, &mut num_processes) } != 0 {
            continue;
        }

        for proc in 0..num_processes {
            if unsafe { xpu_smi::xpu_get_process(proc, &mut infobuf) } != 0 {
                continue;
            }

            let pid = Pid::maybe(infobuf.pid as u64);
            let (username, uid) = ptable.lookup(pid);
            result.push(gpu::Process {
                devices: vec![gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(xpu, dev),
                }],
                pid,
                user: username.clone(),
                uid: uid as Uid,
                mem_pct: infobuf.mem_util as f32,
                gpu_pct: infobuf.gpu_util as f32,
                mem_size_kib: infobuf.mem_size,
                command: None,
            })
        }

        unsafe { xpu_smi::xpu_free_processes() };
    }

    Some(result)
}

fn get_card_uuid(xpu: &XpuGPU, dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: xpu_smi::xpu_card_info_t = Default::default();
    if unsafe { xpu_smi::xpu_device_get_card_info(dev, &mut infobuf) } == 0 {
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
