use crate::gpu::{self, fakegpu_smi};
use crate::ps;

pub struct FakegpuGPU {
    pub hostname: String,
    pub boot_time: u64,
}

pub fn probe(hostname: &str, boot_time: u64) -> Option<Box<dyn gpu::Gpu>> {
    if fakegpu_present() {
        Some(Box::new(FakegpuGPU {
            hostname: hostname.to_string(),
            boot_time,
        }))
    } else {
        None
    }
}

impl gpu::Gpu for FakegpuGPU {
    fn get_card_configuration(&self) -> Result<Vec<gpu::Card>, String> {
        if let Some(info) = fakegpu_smi::get_card_configuration(self) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_process_utilization(
        &self,
        ptable: &ps::ProcessTable,
    ) -> Result<Vec<gpu::Process>, String> {
        if let Some(info) = fakegpu_smi::get_process_utilization(self, ptable) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }

    fn get_card_utilization(&self) -> Result<Vec<gpu::CardState>, String> {
        if let Some(info) = fakegpu_smi::get_card_utilization(self) {
            Ok(info)
        } else {
            Ok(vec![])
        }
    }
}

fn fakegpu_present() -> bool {
    fakegpu_smi::fakegpu_detect()
}
