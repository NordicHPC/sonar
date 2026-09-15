#![allow(clippy::comparison_to_empty)]

use crate::gpu::{self, amd_smi};
use crate::ps;
use crate::types::Pid;
use crate::util::cstrdup::cstrdup;

use std::path::Path;

#[link(name = "sonar-amd", kind = "static")]
unsafe extern "C" {}

pub struct AmdGPU {
    pub hostname: String,
    pub boot_time: u64,
}

pub fn probe(hostname: &str, boot_time: u64) -> Option<Box<dyn gpu::Gpu>> {
    if amd_present() {
        Some(Box::new(AmdGPU {
            hostname: hostname.to_string(),
            boot_time,
        }))
    } else {
        None
    }
}

impl gpu::Gpu for AmdGPU {
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

// On all nodes we've looked at (ML systems, Lumi), /sys/module/amdgpu exists iff there are AMD
// accelerators present.

fn amd_present() -> bool {
    Path::new("/sys/module/amdgpu").exists()
}

// C interface

fn get_card_configuration(amd: &AmdGPU) -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { amd_smi::amdml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: amd_smi::amdml_card_info_t = Default::default();
    for dev in 0..num_devices {
        if unsafe { amd_smi::amdml_device_get_card_info(dev, &mut infobuf) } == 0 {
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
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid_from_info(amd, &infobuf),
                },
                manufacturer: "AMD".to_string(),
                model,
                arch,
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

fn get_card_utilization(amd: &AmdGPU) -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { amd_smi::amdml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: amd_smi::amdml_card_state_t = Default::default();
    for dev in 0..num_devices {
        if unsafe { amd_smi::amdml_device_get_card_state(dev, &mut infobuf) } == 0 {
            result.push(gpu::CardState {
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(amd, dev),
                },
                failing: 0,
                fan_speed_pct: infobuf.fan_speed_pct,
                compute_mode: "".to_string(),
                perf_state: infobuf.perf_level as i64,
                mem_reserved_kib: 0,
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
                    uuid: get_card_uuid(amd, dev),
                },
                failing: gpu::GENERIC_FAILURE,
                ..Default::default()
            })
        }
    }

    Some(result)
}

fn get_process_utilization(amd: &AmdGPU, ptable: &ps::ProcessTable) -> Option<Vec<gpu::Process>> {
    let mut result = vec![];

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { amd_smi::amdml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut infobuf: amd_smi::amdml_gpu_process_t = Default::default();
    let mut num_processes: cty::uint32_t = 0;
    if unsafe { amd_smi::amdml_device_probe_processes(&mut num_processes) } != 0 {
        return None;
    }

    for proc in 0..num_processes {
        if unsafe { amd_smi::amdml_get_process(proc, &mut infobuf) } != 0 {
            continue;
        }

        let pid = Pid::maybe(infobuf.pid as u64);
        let (username, uid) = ptable.lookup(pid);
        let mut indices = infobuf.cards as usize;
        let mut k = 0u32;
        let mut devices = vec![];
        while indices != 0 {
            if (indices & 1) == 1 {
                devices.push(gpu::Name {
                    index: k,
                    uuid: get_card_uuid(amd, k),
                });
            }
            indices >>= 1;
            k += 1;
        }
        result.push(gpu::Process {
            devices,
            pid: Pid::maybe(infobuf.pid as u64),
            user: username,
            uid,
            mem_pct: infobuf.mem_util as f32,
            gpu_pct: infobuf.gpu_util as f32,
            mem_size_kib: (infobuf.mem_size / 1024),
            command: None,
        })
    }

    unsafe { amd_smi::amdml_free_processes() };

    Some(result)
}

fn get_card_uuid(amd: &AmdGPU, dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: amd_smi::amdml_card_info_t = Default::default();
    if unsafe { amd_smi::amdml_device_get_card_info(dev, &mut infobuf) } == 0 {
        get_card_uuid_from_info(amd, &infobuf)
    } else {
        format!("{}/{}/amd#{dev}", &amd.hostname, amd.boot_time)
    }
}

fn get_card_uuid_from_info(amd: &AmdGPU, infobuf: &amd_smi::amdml_card_info_t) -> String {
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
            &amd.hostname,
            amd.boot_time,
            cstrdup(&infobuf.bus_addr)
        )
    }
}
