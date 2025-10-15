use crate::gpu::{Card, CardState, Gpu, GpuAPI, Process};
use crate::ps;

#[cfg(feature = "amd")]
use crate::gpu::amd;
#[cfg(feature = "habana")]
use crate::gpu::habana;
#[cfg(feature = "nvidia")]
use crate::gpu::nvidia;
#[cfg(feature = "xpu")]
use crate::gpu::xpu;

pub struct RealGpu {
    #[allow(dead_code)]
    hostname: String,
    #[allow(dead_code)]
    boot_time: u64,
}

impl RealGpu {
    pub fn new(hostname: String, boot_time: u64) -> RealGpu {
        RealGpu {
            hostname,
            boot_time,
        }
    }
}

impl GpuAPI for RealGpu {
    fn probe(&self) -> Option<Box<dyn Gpu>> {
        let mut gpus = vec![];
        #[cfg(feature = "nvidia")]
        if let Some(nvidia) = nvidia::probe() {
            gpus.push(nvidia);
        }
        #[cfg(feature = "amd")]
        if let Some(amd) = amd::probe(&self.hostname, self.boot_time) {
            gpus.push(amd);
        }
        #[cfg(feature = "xpu")]
        if let Some(xpu) = xpu::probe(&self.hostname, self.boot_time) {
            gpus.push(xpu);
        }
        #[cfg(feature = "habana")]
        if let Some(habana) = habana::probe() {
            gpus.push(habana);
        }
        match gpus.len() {
            0 => None,
            1 => Some(gpus.remove(0)),
            _ => Some(Box::new(MultiGpu { gpus })),
        }
    }
}

pub struct MultiGpu {
    gpus: Vec<Box<dyn Gpu>>,
}

impl Gpu for MultiGpu {
    fn get_card_configuration(&self) -> Result<Vec<Card>, String> {
        let mut cs = vec![];
        for c in &self.gpus {
            match c.get_card_configuration() {
                Ok(mut xs) => {
                    cs.append(&mut xs);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(cs)
    }

    fn get_process_utilization(
        &self,
        user_by_pid: &ps::ProcessTable,
    ) -> Result<Vec<Process>, String> {
        let mut ps = vec![];
        for c in &self.gpus {
            match c.get_process_utilization(user_by_pid) {
                Ok(mut xs) => {
                    ps.append(&mut xs);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(ps)
    }

    fn get_card_utilization(&self) -> Result<Vec<CardState>, String> {
        let mut cs = vec![];
        for c in &self.gpus {
            match c.get_card_utilization() {
                Ok(mut xs) => {
                    cs.append(&mut xs);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(cs)
    }
}
