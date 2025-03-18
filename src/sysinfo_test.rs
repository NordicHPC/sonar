#[allow(unused_imports)]
use crate::sysinfo;
#[allow(unused_imports)]
use crate::mocksystem;
#[allow(unused_imports)]
use crate::gpuapi;

#[allow(unused_imports)]
use std::collections::HashMap;

// Test that an error field is added correctly if we fail to obtain information we must have.

#[test]
pub fn sysinfo_error_test() {
    // Empty API should cause get_cpu_info to fail and there should be an error field.
    let info = sysinfo::compute_sysinfo(&mocksystem::MockSystem::new().freeze());
    assert!(info.get("error").is_some());
}

// Test that the output is the expected output

#[cfg(target_arch = "x86_64")]  // the mock cpuinfo files are x86_64-specific
#[test]
pub fn sysinfo_output_test() {
    let mut files = HashMap::new();
    files.insert("cpuinfo".to_string(), std::include_str!("testdata/cpuinfo-x86_64.txt").to_string());
    files.insert(
        "meminfo".to_string(),
        "MemTotal:       16093776 kB".to_string(),
    );
    let system = mocksystem::MockSystem::new()
        .with_version("0.13.100")
        .with_timestamp("2025-02-11T08:47+01:00")
        .with_hostname("yes.no")
        .with_files(files)
        .with_card(gpuapi::Card{
            bus_addr: "12:14:16".to_string(),
            device: gpuapi::GpuName {
                index: 0,
                uuid: "1234.5678".to_string(),
            },
            model: "Yoyodyne 1".to_string(),
            mem_size_kib: 1024*1024,
            power_limit_watt: 2000,
            max_power_limit_watt: 3000,
            max_ce_clock_mhz: 100000,
            ..Default::default()
        })
        .freeze();
    // CSV
    let mut output = Vec::new();
    sysinfo::show_system(&mut output, &system, true, false);
    let info = String::from_utf8_lossy(&output);
    let expect = r#"version=0.13.100,timestamp=2025-02-11T08:47+01:00,hostname=yes.no,"description=2x4 (hyperthreaded) Intel(R) Xeon(R) CPU E5-2637 v4 @ 3.50GHz, 15 GiB, 1x Yoyodyne 1 @ 1GiB",cpu_cores=16,mem_gb=15,gpu_cards=1,gpumem_gb=1,"gpu_info=""bus_addr=12:14:16,index=0,uuid=1234.5678,""""manufacturer=Yoyodyne, Inc."""",model=Yoyodyne 1,arch=,driver=,firmware=,mem_size_kib=1048576,power_limit_watt=2000,max_power_limit_watt=3000,min_power_limit_watt=0,max_ce_clock_mhz=100000,max_mem_clock_mhz=0"""
"#;
    assert!(info == expect);

    // JSON
    let mut output = Vec::new();
    sysinfo::show_system(&mut output, &system, false, false);
    let info = String::from_utf8_lossy(&output);
    let expect = r#"{"version":"0.13.100","timestamp":"2025-02-11T08:47+01:00","hostname":"yes.no","description":"2x4 (hyperthreaded) Intel(R) Xeon(R) CPU E5-2637 v4 @ 3.50GHz, 15 GiB, 1x Yoyodyne 1 @ 1GiB","cpu_cores":16,"mem_gb":15,"gpu_cards":1,"gpumem_gb":1,"gpu_info":[{"bus_addr":"12:14:16","index":0,"uuid":"1234.5678","manufacturer":"Yoyodyne, Inc.","model":"Yoyodyne 1","arch":"","driver":"","firmware":"","mem_size_kib":1048576,"power_limit_watt":2000,"max_power_limit_watt":3000,"min_power_limit_watt":0,"max_ce_clock_mhz":100000,"max_mem_clock_mhz":0}]}
"#;
    assert!(info == expect);
}
