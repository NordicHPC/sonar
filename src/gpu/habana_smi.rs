// Rust wrapper around ../../gpuapi/sonar-habana.{c,h}.

use crate::gpu;
use crate::util::cstrdup;

////// C library API //////////////////////////////////////////////////////////////////////////////

// The data structures and signatures defined here must be exactly those defined in the header file,
// using types from `cty`.  See ../../gpuapi/sonar-habana.h for all documentation of functionality and
// units.
//
// TODO: We should use bindgen for this but not important at the moment.

#[link(name = "sonar-habana", kind = "static")]
extern "C" {
    pub fn habana_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

#[repr(C)]
pub struct HabanaCardInfo {
    bus_addr: [cty::c_char; 256],
    model: [cty::c_char; 256],
    driver: [cty::c_char; 256],
    firmware: [cty::c_char; 256],
    uuid: [cty::c_char; 256],
    totalmem: cty::uint64_t,
    max_ce_clock: cty::c_uint,
    max_power_limit: cty::c_uint,
}

impl Default for HabanaCardInfo {
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

#[link(name = "sonar-habana", kind = "static")]
extern "C" {
    pub fn habana_device_get_card_info(
        device: cty::uint32_t,
        buf: *mut HabanaCardInfo,
    ) -> cty::c_int;
}

const PERF_STATE_UNKNOWN: cty::c_int = -1;

#[repr(C)]
#[derive(Default)]
pub struct HabanaCardState {
    perf_state: cty::c_int,
    gpu_util: cty::c_float,
    mem_util: cty::c_float,
    mem_used: cty::uint64_t,
    temp: cty::c_uint,
    power: cty::c_uint,
    ce_clock: cty::c_uint,
}

#[link(name = "sonar-habana", kind = "static")]
extern "C" {
    pub fn habana_device_get_card_state(
        device: cty::uint32_t,
        buf: *mut HabanaCardState,
    ) -> cty::c_int;
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn get_card_configuration() -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { habana_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: HabanaCardInfo = Default::default();
    for dev in 0..num_devices {
        if unsafe { habana_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                device: gpu::Name {
                    index: dev,
                    uuid: get_card_uuid(dev),
                },
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

pub fn get_card_utilization() -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { habana_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: HabanaCardState = Default::default();
    for dev in 0..num_devices {
        if unsafe { habana_device_get_card_state(dev, &mut infobuf) } == 0 {
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
    let mut infobuf: HabanaCardInfo = Default::default();
    if unsafe { habana_device_get_card_info(dev, &mut infobuf) } == 0 {
        cstrdup(&infobuf.uuid)
    } else {
        "".to_string()
    }
}
