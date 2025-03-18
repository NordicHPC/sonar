// This is stub code, included to test the feature system, to be fleshed out later.
// If you enable the xpu feature, you'll get a link error because there's no XPU gpuapi adapter.

use crate::gpuapi;
use crate::ps;

pub struct XpuGPU {}

pub fn probe() -> Option<Box<dyn gpuapi::GPU>> {
    if xpu_present() {
        Some(Box::new(XpuGPU {}))
    } else {
        None
    }
}

impl gpuapi::GPU for XpuGPU {
    fn get_manufacturer(&mut self) -> String {
        "Intel".to_string()
    }

    fn get_card_configuration(&mut self) -> Result<Vec<gpuapi::Card>, String> {
        let mut num_devices: cty::uint32_t = 0;
        if unsafe { xpu_device_get_count(&mut num_devices) } != 0 {
            return Ok(vec![]);
        }
        return Ok(vec![]);
    }

    fn get_process_utilization(
        &mut self,
        _ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpuapi::Process>, String> {
        Ok(vec![])
    }

    fn get_card_utilization(&mut self) -> Result<Vec<gpuapi::CardState>, String> {
        Ok(vec![])
    }
}

fn xpu_present() -> bool {
    false
}

#[link(name = "sonar-xpu", kind = "static")]
extern "C" {
    pub fn xpu_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}
