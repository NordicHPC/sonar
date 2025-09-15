use crate::gpu::{self, habana_smi};
use crate::ps;

use std::path::Path;

pub struct HabanaGPU {}

pub fn probe() -> Option<Box<dyn gpu::Gpu>> {
    if habana_present() {
        Some(Box::new(HabanaGPU {}))
    } else {
        None
    }
}

impl gpu::Gpu for HabanaGPU {
    fn get_manufacturer(&self) -> String {
        "Intel".to_string()
    }

    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = habana_smi::get_card_configuration() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        if let Some(info) = habana_smi::get_card_utilization() {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    // We don't have this information on Habana yet, maybe not ever.  For now, return
    // an error, and let the caller sort it out.
    fn get_process_utilization(
        &self,
        _ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        Err("No per-process utilization information".to_string())
    }
}

fn habana_present() -> bool {
    Path::new("/sys/module/habanalabs").exists()
}
