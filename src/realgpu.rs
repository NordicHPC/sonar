use crate::gpuapi::{GpuAPI, GPU};

#[cfg(feature = "amd")]
use crate::amd;
#[cfg(feature = "nvidia")]
use crate::nvidia;
#[cfg(feature = "xpu")]
use crate::xpu;

pub struct RealGpu {}

impl RealGpu {
    pub fn new() -> RealGpu {
        RealGpu {}
    }
}

impl GpuAPI for RealGpu {
    fn probe(&self) -> Option<Box<dyn GPU>> {
        #[cfg(feature = "nvidia")]
        if let Some(nvidia) = nvidia::probe() {
            return Some(nvidia);
        }
        #[cfg(feature = "amd")]
        if let Some(amd) = amd::probe() {
            return Some(amd);
        }
        #[cfg(feature = "xpu")]
        if let Some(xpu) = xpu::probe() {
            return Some(xpu);
        }
        None
    }
}
