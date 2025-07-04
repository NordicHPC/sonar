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
        ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = xpu_smi::get_process_utilization(ptable) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        if let Some(info) = xpu_smi::get_card_utilization() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }
}

// Probably this, though actually hard to figure out exactly.  Looking at strings in the smi
// library, i915 is definitely being looked for, and some other output also indicates this is what
// we want.

fn xpu_present() -> bool {
    Path::new("/sys/module/i915").exists()
}
