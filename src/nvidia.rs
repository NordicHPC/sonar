use crate::gpuapi;
use crate::nvidia_nvml;
use crate::ps;

use std::path::Path;

pub struct NvidiaGPU {}

pub fn probe() -> Option<Box<dyn gpuapi::GPU>> {
    if nvidia_present() {
        Some(Box::new(NvidiaGPU {}))
    } else {
        None
    }
}

impl gpuapi::GPU for NvidiaGPU {
    fn get_manufacturer(&mut self) -> String {
        "NVIDIA".to_string()
    }

    fn get_card_configuration(&mut self) -> Result<Vec<gpuapi::Card>, String> {
        if let Some(info) = nvidia_nvml::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &mut self,
        user_by_pid: &ps::UserTable,
    ) -> Result<Vec<gpuapi::Process>, String> {
        if let Some(info) = nvidia_nvml::get_process_utilization(user_by_pid) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&mut self) -> Result<Vec<gpuapi::CardState>, String> {
        if let Some(info) = nvidia_nvml::get_card_utilization() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }
}

// On all nodes we've looked at (Fox, Betzy, ML systems), /sys/module/nvidia exists iff there are
// nvidia accelerators present.

fn nvidia_present() -> bool {
    return Path::new("/sys/module/nvidia").exists();
}
