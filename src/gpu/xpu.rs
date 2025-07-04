// This is stub code, included to test the feature system, to be fleshed out later.
// If you enable the xpu feature, you'll get a link error because there's no XPU gpuapi adapter.

use crate::gpu::{self, xpu_smi};
use crate::ps;

use std::path::Path;

pub struct XpuGPU {}

pub fn probe() -> Option<Box<dyn gpu::Gpu>> {
    if xpu_present() {
        Some(Box::new(XpuGPU {}))
    } else {
        None
    }
}

impl gpu::Gpu for XpuGPU {
    fn get_manufacturer(&self) -> String {
        "Intel".to_string()
    }

    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = xpu_smi::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &self,
        _ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        Ok(vec![])
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        Ok(vec![])
    }
}

fn xpu_present() -> bool {
    // Probably this, though actually hard to figure out.
    Path::new("/sys/module/i915").exists()
}
