// Rust wrapper around ../../gpuapi/sonar-xpu.{c,h}.

use crate::gpu;
use crate::ps;
use crate::util::cstrdup;

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
    uuid: [cty::c_char; 256],
}

impl Default for XpuCardInfo {
    fn default() -> Self {
        Self {
            bus_addr: [0; 256],
            model: [0; 256],
            uuid: [0; 256],
        }
    }
}

#[link(name = "sonar-xpu", kind = "static")]
extern "C" {
    pub fn xpu_device_get_card_info(device: cty::uint32_t, buf: *mut XpuCardInfo) -> cty::c_int;
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn get_card_configuration() -> Option<Vec<gpu::Card>> {
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
                model: cstrdup(&infobuf.model),
                device: gpu::Name {
                    index: dev,
                    uuid: cstrdup(&infobuf.uuid),
                },
                ..Default::default()
            })
        }
    }

    Some(result)
}

pub fn get_card_utilization() -> Option<Vec<gpu::CardState>> {
    None
}

pub fn get_process_utilization(_ptable: &ps::ProcessTable) -> Option<Vec<gpu::Process>> {
    None
}

