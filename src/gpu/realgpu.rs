use crate::gpu::{Gpu, GpuAPI};

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
        #[cfg(feature = "nvidia")]
        if let Some(nvidia) = nvidia::probe() {
            return Some(nvidia);
        }
        #[cfg(feature = "amd")]
        if let Some(amd) = amd::probe(&self.hostname, self.boot_time) {
            return Some(amd);
        }
        #[cfg(feature = "xpu")]
        if let Some(xpu) = xpu::probe(&self.hostname, self.boot_time) {
            return Some(xpu);
        }
        #[cfg(feature = "habana")]
        if let Some(habana) = habana::probe() {
            return Some(habana);
        }
        None
    }
}
