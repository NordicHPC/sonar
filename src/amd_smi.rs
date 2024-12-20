use crate::gpu;
use crate::ps::UserTable;
use crate::util::cstrdup;

////// C library API //////////////////////////////////////////////////////////////////////////////

// These APIs must match the C APIs *exactly*.  See ../gpuapi/sonar-nvidia.h for documentation of
// functionality and units.

// Should use bindgen for this but not important yet.

extern "C" {
    pub fn amdml_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn test() {
    let mut num_devices: cty::uint32_t = 0;
    let v = unsafe { amdml_device_get_count(&mut num_devices) };
    println!("v={v}, num_devices={num_devices}");
}
