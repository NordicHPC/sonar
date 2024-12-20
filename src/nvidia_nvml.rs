use crate::gpu;
use crate::ps::UserTable;
use crate::util::cstrdup;

////// C library API //////////////////////////////////////////////////////////////////////////////

// These APIs must match the C APIs *exactly*.  See ../gpuapi/sonar-nvidia.h for documentation of
// functionality and units.

// Should use bindgen for this but not important yet.

extern "C" {
    pub fn nvml_device_get_count(count: *mut cty::uint32_t) -> cty::c_int;
}

#[repr(C)]
pub struct NvmlCardInfo {
    bus_addr: [cty::c_char; 80],
    model: [cty::c_char; 96],
    architecture: [cty::c_char; 32],
    driver: [cty::c_char; 80],
    firmware: [cty::c_char; 10],
    uuid: [cty::c_char; 96],
    mem_total: cty::uint64_t,
    power_limit: cty::c_uint,
    min_power_limit: cty::c_uint,
    max_power_limit: cty::c_uint,
    max_ce_clock: cty::c_uint,
    max_mem_clock: cty::c_uint,
}

impl Default for NvmlCardInfo {
    fn default() -> Self {
        Self {
            bus_addr: [0; 80],
            model: [0; 96],
            architecture: [0; 32],
            driver: [0; 80],
            firmware: [0; 10],
            uuid: [0; 96],
            mem_total: 0,
            power_limit: 0,
            min_power_limit: 0,
            max_power_limit: 0,
            max_ce_clock: 0,
            max_mem_clock: 0,
        }
    }
}

extern "C" {
    pub fn nvml_device_get_card_info(device: cty::uint32_t, buf: *mut NvmlCardInfo) -> cty::c_int;
}

#[repr(C)]
pub struct NvmlCardState {
    fan_speed: cty::c_uint,
    compute_mode: [cty::c_char; 32],
    perf_state: [cty::c_char; 8],
    mem_reserved: cty::uint64_t,
    mem_used: cty::uint64_t,
    gpu_util: cty::c_float,
    mem_util: cty::c_float,
    temp: cty::c_uint,
    power: cty::c_uint,
    power_limit: cty::c_uint,
    ce_clock: cty::c_uint,
    mem_clock: cty::c_uint,
}

impl Default for NvmlCardState {
    fn default() -> Self {
        Self {
            fan_speed: 0,
            compute_mode: [0; 32],
            perf_state: [0; 8],
            mem_reserved: 0,
            mem_used: 0,
            gpu_util: 0.0,
            mem_util: 0.0,
            temp: 0,
            power: 0,
            power_limit: 0,
            ce_clock: 0,
            mem_clock: 0,
        }
    }
}

extern "C" {
    pub fn nvml_device_get_card_state(device: cty::uint32_t, buf: *mut NvmlCardState)
        -> cty::c_int;
}

#[repr(C)]
pub struct NvmlGpuProcess {
    pid: cty::uint32_t,
    mem_util: cty::uint32_t,
    gpu_util: cty::uint32_t,
    mem_size: cty::uint64_t,
}

impl Default for NvmlGpuProcess {
    fn default() -> Self {
        Self {
            pid: 0,
            mem_util: 0,
            gpu_util: 0,
            mem_size: 0,
        }
    }
}

extern "C" {
    pub fn nvml_device_probe_processes(
        device: cty::uint32_t,
        count: *mut cty::uint32_t,
    ) -> cty::c_int;
    pub fn nvml_get_process(index: cty::uint32_t, buf: *mut NvmlGpuProcess) -> cty::c_int;
    pub fn nvml_free_processes();
}

////// End C library API //////////////////////////////////////////////////////////////////////////

pub fn get_card_configuration() -> Option<Vec<gpu::Card>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: NvmlCardInfo = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvml_device_get_card_info(dev, &mut infobuf) } == 0 {
            result.push(gpu::Card {
                bus_addr: cstrdup(&infobuf.bus_addr),
                index: dev as i32,
                model: cstrdup(&infobuf.model),
                arch: cstrdup(&infobuf.architecture),
                driver: cstrdup(&infobuf.driver),
                firmware: cstrdup(&infobuf.firmware),
                uuid: cstrdup(&infobuf.uuid),
                mem_size_kib: (infobuf.mem_total / 1024) as i64,
                power_limit_watt: (infobuf.power_limit / 1000) as i32,
                max_power_limit_watt: (infobuf.min_power_limit / 1000) as i32,
                min_power_limit_watt: (infobuf.max_power_limit / 1000) as i32,
                max_ce_clock_mhz: infobuf.max_ce_clock as i32,
                max_mem_clock_mhz: infobuf.max_mem_clock as i32,
            })
        }
    }

    Some(result)
}

pub fn get_card_utilization() -> Option<Vec<gpu::CardState>> {
    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    let mut result = vec![];
    let mut infobuf: NvmlCardState = Default::default();
    for dev in 0..num_devices {
        if unsafe { nvml_device_get_card_state(dev, &mut infobuf) } == 0 {
            result.push(gpu::CardState {
                index: dev as i32,
                fan_speed_pct: infobuf.fan_speed as f32,
                compute_mode: cstrdup(&infobuf.compute_mode),
                perf_state: cstrdup(&infobuf.perf_state),
                mem_reserved_kib: (infobuf.mem_reserved / 1024) as i64,
                mem_used_kib: (infobuf.mem_used / 1024) as i64,
                gpu_utilization_pct: infobuf.gpu_util,
                mem_utilization_pct: infobuf.mem_util,
                temp_c: infobuf.temp as i32,
                power_watt: (infobuf.power / 1000) as i32,
                power_limit_watt: (infobuf.power_limit / 1000) as i32,
                ce_clock_mhz: infobuf.ce_clock as i32,
                mem_clock_mhz: infobuf.mem_clock as i32,
            })
        }
    }

    Some(result)
}

// We get these from the device:
//
// - device index
// - pid
// - gpu_pct
// - mem_pct
// - mem_size_kib
//
// We get these from user_by_pid:
//
// - uid
// - user name
//
// That leaves:
//
// - command
//
// The command is most easily gotten from the pid: we look it up in procfs.  Really not clear why we
// would want the GPU to supply the command name.  It does not appear that the GPU has this
// information anyway.  So we can make it an option.

pub fn get_process_utilization(user_by_pid: &UserTable) -> Option<Vec<gpu::Process>> {
    let mut result = vec![];

    let mut num_devices: cty::uint32_t = 0;
    if unsafe { nvml_device_get_count(&mut num_devices) } != 0 {
        return None;
    }

    println!("{num_devices} devices");

    let mut infobuf: NvmlGpuProcess = Default::default();
    for dev in 0..num_devices {
        let mut num_processes: cty::uint32_t = 0;
        if unsafe { nvml_device_probe_processes(dev, &mut num_processes) } != 0 {
            println!("probe_processes found 0 processes");
            continue;
        }

        println!("{num_processes} processes");

        for proc in 0..num_processes {
            if unsafe { nvml_get_process(proc, &mut infobuf) } != 0 {
                println!("  get_process {proc} failed");
                continue;
            }

            let (username, uid) = match user_by_pid.get(&(infobuf.pid as usize)) {
                Some(x) => *x,
                None => ("_unknown_", 1),
            };
            println!("  pid {} mem% {} gpu% {} memkb {}", infobuf.pid, infobuf.mem_util, infobuf.gpu_util, infobuf.mem_size);
            result.push(gpu::Process{
                device: Some(dev as usize),
                pid: infobuf.pid as usize,
                user: username.to_string(),
                uid: uid,
                mem_pct: infobuf.mem_util as f64,
                gpu_pct: infobuf.gpu_util as f64,
                mem_size_kib: infobuf.mem_size as usize,
                command: None,
            })
        }

        unsafe { nvml_free_processes() };
    }

    Some(result)
}
