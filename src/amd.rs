use crate::amd_smi;
use crate::gpuapi;
use crate::ps;

use std::path::Path;

pub struct AmdGPU {}

pub fn probe() -> Option<Box<dyn gpuapi::GPU>> {
    if amd_present() {
        Some(Box::new(AmdGPU {}))
    } else {
        None
    }
}

impl gpuapi::GPU for AmdGPU {
    fn get_manufacturer(&mut self) -> String {
        "AMD".to_string()
    }

    fn get_card_configuration(&mut self) -> Result<Vec<gpuapi::Card>, String> {
        if let Some(info) = amd_smi::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &mut self,
        user_by_pid: &ps::UserTable,
    ) -> Result<Vec<gpuapi::Process>, String> {
        if let Some(info) = amd_smi::get_process_utilization(user_by_pid) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&mut self) -> Result<Vec<gpuapi::CardState>, String> {
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
