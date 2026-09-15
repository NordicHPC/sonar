use crate::gpu::{self, habana_smi};
use crate::ps;
use crate::util::cstrdup::cstrdup;

use std::path::Path;

#[link(name = "sonar-habana", kind = "static")]
unsafe extern "C" {}

pub struct HabanaGPU {}

pub fn probe() -> Option<Box<dyn gpu::Gpu>> {
    if habana_present() {
        Some(Box::new(HabanaGPU {}))
    } else {
        None
    }
}

impl gpu::Gpu for HabanaGPU {
    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = get_card_configuration() {
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

    // We don't have this information on Habana yet, maybe not ever.  For now, return
    // an error, and let the caller sort it out.
    fn get_process_utilization(
        &self,
        _ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        Err("No per-process utilization information".to_string())
    }
}

fn habana_present() -> bool {
    Path::new("/sys/module/habanalabs").exists()
}

// C interface

const PERF_STATE_UNKNOWN: cty::c_int = -1;

fn get_card_configuration() -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { habana_smi::habana_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: habana_smi::habana_card_info_t = Default::default();
    for dev in 0..num_devices {
        if unsafe { habana_smi::habana_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(dev),
                },
                manufacturer: "Intel".to_string(),
                model: cstrdup(&infobuf.model),
                arch: "Habana".to_string(),
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                mem_size_kib: (infobuf.totalmem / 1024),
                max_ce_clock_mhz: infobuf.max_ce_clock,
                max_mem_clock_mhz: 0,
                power_limit_watt: 0,
                max_power_limit_watt: infobuf.max_power_limit,
                min_power_limit_watt: 0,
            })
        }
    }

    Some(result)
}

fn get_card_utilization() -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { habana_smi::habana_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: habana_smi::habana_card_state_t = Default::default();
    for dev in 0..num_devices {
        if unsafe { habana_smi::habana_device_get_card_state(dev, &mut infobuf) } == 0 {
            let perf = match infobuf.perf_state {
                PERF_STATE_UNKNOWN => -1,
                x => x,
            };
            result.push(gpu::CardState {
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(dev),
                },
                gpu_utilization_pct: infobuf.gpu_util,
                mem_utilization_pct: infobuf.mem_util,
                mem_used_kib: (infobuf.mem_used / 1024),
                temp_c: infobuf.temp,
                power_watt: (infobuf.power / 1000),
                ce_clock_mhz: infobuf.ce_clock,
                perf_state: perf as i64,
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
                    uuid: get_card_uuid(dev),
                },
                failing: gpu::GENERIC_FAILURE,
                ..Default::default()
            })
        }
    }

    Some(result)
}

fn get_card_uuid(dev: u32) -> String {
    // TODO: Not the most efficient way to do it, but OK for now?
    let mut infobuf: habana_smi::habana_card_info_t = Default::default();
    if unsafe { habana_smi::habana_device_get_card_info(dev, &mut infobuf) } == 0 {
        cstrdup(&infobuf.uuid)
    } else {
        "".to_string()
    }
}
