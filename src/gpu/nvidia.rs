use crate::gpu::{self, nvidia_nvml};
use crate::ps;

use std::path::Path;

pub struct NvidiaGPU {}

pub fn probe() -> Option<Box<dyn gpu::Gpu>> {
    if nvidia_present() {
        Some(Box::new(NvidiaGPU {}))
    } else {
        None
    }
}

impl gpu::Gpu for NvidiaGPU {
    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = nvidia_nvml::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &self,
        ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = nvidia_nvml::get_process_utilization(ptable) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
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
    Path::new("/sys/module/nvidia").exists()
}
