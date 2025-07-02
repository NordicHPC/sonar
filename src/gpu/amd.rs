use crate::gpu::{self, amd_smi};
use crate::ps;

use std::path::Path;

pub struct AmdGPU {}

pub fn probe() -> Option<Box<dyn gpu::Gpu>> {
    if amd_present() {
        Some(Box::new(AmdGPU {}))
    } else {
        None
    }
}

impl gpu::Gpu for AmdGPU {
    fn get_manufacturer(&self) -> String {
        "AMD".to_string()
    }

    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = amd_smi::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &self,
        ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = amd_smi::get_process_utilization(ptable) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        if let Some(info) = amd_smi::get_card_utilization() {
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
