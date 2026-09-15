use crate::gpu::{self, nvidia_nvml};
use crate::ps;
use crate::types::{Pid, Uid};
use crate::util::cstrdup::cstrdup;

use std::path::Path;

#[link(name = "sonar-nvidia", kind = "static")]
unsafe extern "C" {}

pub struct NvidiaGPU {}

pub fn probe() -> Option<Box<dyn gpu::Gpu>> {
    if nvidia_present() {
        Some(Box::new(NvidiaGPU {}))
    } else {
        None
    }
}

impl gpu::Gpu for NvidiaGPU {
    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &self,
        ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = get_process_utilization(ptable) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        if let Some(info) = get_card_utilization() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }
}

// On all nodes we've looked at (Fox, Betzy, ML systems), /sys/module/nvidia exists iff there are
// nvidia accelerators present.

fn nvidia_present() -> bool {
    Path::new("/sys/module/nvidia").exists()
}

// C interface

const COMP_MODE_UNKNOWN: cty::c_int = -1;
const COMP_MODE_DEFAULT: cty::c_int = 0;
const COMP_MODE_PROHIBITED: cty::c_int = 1;
const COMP_MODE_EXCLUSIVE_PROCESS: cty::c_int = 2;

const PERF_STATE_UNKNOWN: cty::c_int = -1;

fn get_card_configuration() -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvidia_nvml::nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: nvidia_nvml::nvml_card_info = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvidia_nvml::nvml_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid_from_info(&infobuf),
                },
                manufacturer: "NVIDIA".to_string(),
                model: cstrdup(&infobuf.model),
                arch: cstrdup(&infobuf.architecture),
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                mem_size_kib: (infobuf.totalmem / 1024),
                power_limit_watt: (infobuf.power_limit / 1000),
                max_power_limit_watt: (infobuf.max_power_limit / 1000),
                min_power_limit_watt: (infobuf.min_power_limit / 1000),
                max_ce_clock_mhz: infobuf.max_ce_clock,
                max_mem_clock_mhz: infobuf.max_mem_clock,
            })
        }
    }

    Some(result)
}

fn get_card_utilization() -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvidia_nvml::nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: nvidia_nvml::nvml_card_state = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvidia_nvml::nvml_device_get_card_state(dev, &mut infobuf) } == 0 {
            let mode = match infobuf.compute_mode {
                COMP_MODE_DEFAULT => "",
                COMP_MODE_PROHIBITED => "Prohibited",
                COMP_MODE_EXCLUSIVE_PROCESS => "ExclusiveProcess",
                COMP_MODE_UNKNOWN => "Unknown",
                _ => "Unknown",
            };
            let perf = match infobuf.perf_state {
                PERF_STATE_UNKNOWN => -1,
                x => x,
            };
            result.push(gpu::CardState {
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(dev),
                },
                failing: 0,
                fan_speed_pct: infobuf.fan_speed as f32,
                compute_mode: mode.to_string(),
                perf_state: perf as i64,
                mem_reserved_kib: (infobuf.mem_reserved / 1024),
                mem_used_kib: (infobuf.mem_used / 1024),
                gpu_utilization_pct: infobuf.gpu_util,
                mem_utilization_pct: infobuf.mem_util,
                temp_c: infobuf.temp,
                power_watt: (infobuf.power / 1000),
                power_limit_watt: (infobuf.power_limit / 1000),
                ce_clock_mhz: infobuf.ce_clock,
                mem_clock_mhz: infobuf.mem_clock,
            })
        } else {
            result.push(gpu::CardState {
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(dev),
                },
                failing: gpu::GENERIC_FAILURE,
                ..Default::default()
            })
        }
    }

    Some(result)
}

fn get_process_utilization(ptable: &ps::ProcessTable) -> Option<Vec<gpu::Process>> {
    let mut result = vec![];

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvidia_nvml::nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut infobuf: nvidia_nvml::nvml_gpu_process = Default::default();
    for dev in 0..num_devices {
        let mut num_processes: cty::uint32_t = 0;
        if unsafe { nvidia_nvml::nvml_device_probe_processes(dev, &mut num_processes) } != 0 {
            continue;
        }

        for proc in 0..num_processes {
            if unsafe { nvidia_nvml::nvml_get_process(proc, &mut infobuf) } != 0 {
                continue;
            }

            let (username, uid) = ptable.lookup(infobuf.pid as Pid);
            result.push(gpu::Process {
                devices: vec![gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(dev),
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

        unsafe { nvidia_nvml::nvml_free_processes() };
    }

    Some(result)
}

fn get_card_uuid(dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: nvidia_nvml::nvml_card_info = Default::default();
    if unsafe { nvidia_nvml::nvml_device_get_card_info(dev, &mut infobuf) } == 0 {
        get_card_uuid_from_info(&infobuf)
    } else {
        "".to_string()
    }
}

fn get_card_uuid_from_info(infobuf: &nvidia_nvml::nvml_card_info) -> String {
    cstrdup(&infobuf.uuid)
}
