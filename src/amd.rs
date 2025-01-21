// Get info about AMD graphics cards by parsing the output of rocm-smi.
//
// This is pretty hacky!  Something better than this is likely needed and hopefully possible.

use crate::amd_smi;
use crate::gpu;
use crate::ps;

use std::path::Path;

pub struct AmdGPU {}

pub fn probe() -> Option<Box<dyn gpu::GPU>> {
    if amd_present() {
        Some(Box::new(AmdGPU {}))
    } else {
        None
    }
}

impl gpu::GPU for AmdGPU {
    fn get_manufacturer(&mut self) -> String {
        "AMD".to_string()
    }

    fn get_card_configuration(&mut self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = amd_smi::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &mut self,
        user_by_pid: &ps::UserTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = amd_smi::get_process_utilization(user_by_pid) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&mut self) -> Result<Vec<gpu::CardState>, String> {
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
