#[allow(unused_imports)]
use crate::gpu;
#[allow(unused_imports)]
use crate::linux::mocksystem;
#[allow(unused_imports)]
use crate::sysinfo;

#[allow(unused_imports)]
use std::collections::HashMap;

// Test that the output is the expected output

#[cfg(target_arch = "x86_64")] // the mock cpuinfo files are x86_64-specific and linux-specific
#[test]
pub fn sysinfo_output_test() {
    // FIXME: Information leakage!!
    let mut proc_files = HashMap::new();
    proc_files.insert(
        "cpuinfo".to_string(),
        std::include_str!("linux/testdata/cpuinfo-x86_64.txt").to_string(),
    );
    proc_files.insert(
        "meminfo".to_string(),
        "MemTotal:       16093776 kB".to_string(),
    );
    // Really we want something here where we have multiple nodes, but we *also* want sockets>nodes
    // to test that bit.  However I don't want to change the test data on release_0_16 too much so
    // for now let's be happy with sockets==nodes.
    let mut node_files = HashMap::new();
    node_files.insert("node0/distance".to_string(), "10 21".to_string());
    node_files.insert("node1/distance".to_string(), "21 10".to_string());
    let system = mocksystem::Builder::new()
        .with_version("0.13.100")
        .with_timestamp("2025-02-11T08:47+01:00")
        .with_hostname("yes.no")
        .with_cluster("kain.uio.no")
        .with_os("CP/M", "2.2")
        .with_architecture("Z80")
        .with_proc_files(proc_files)
        .with_node_files(node_files)
        .with_card(gpu::Card {
            bus_addr: "12:14:16".to_string(),
            device: gpu::Name {
                index: 0,
                uuid: "1234.5678".to_string(),
            },
            manufacturer: "Yoyodyne, Inc.".to_string(),
            model: "Yoyodyne 1".to_string(),
            mem_size_kib: 1024 * 1024,
            power_limit_watt: 2000,
            max_power_limit_watt: 3000,
            max_ce_clock_mhz: 100000,
            ..Default::default()
        })
        .freeze();

    let mut output = Vec::new();
    sysinfo::show_system(&mut output, &system, "".to_string(), sysinfo::Format::JSON);
    let info = String::from_utf8_lossy(&output);
    let expect = r#"
{"meta":
{"producer":"sonar","version":"0.13.100"},
"data":
{
"type":"sysinfo",
"attributes":
{
"time":"2025-02-11T08:47+01:00",
"cluster":"kain.uio.no",
"node":"yes.no",
"os_name":"CP/M",
"os_release":"2.2",
"numa_nodes":2,
"sockets":2,
"cores_per_socket":4,
"threads_per_core":2,
"cpu_model":"Intel(R) Xeon(R) CPU E5-2637 v4 @ 3.50GHz",
"architecture":"Z80",
"memory":16093776,
"distances":[[10,21],[21,10]],
"cards":[
{
"index":0,
"uuid":"1234.5678",
"address":"12:14:16",
"manufacturer":"Yoyodyne, Inc.",
"model":"Yoyodyne 1",
"memory":1048576,
"power_limit":2000,
"max_power_limit":3000,
"max_ce_clock":100000
}
]
}
}
}
"#;
    // println!("{}", info.replace('\n',""));
    // println!("{}", expect.replace('\n',""));
    assert!(info.replace('\n', "") == expect.replace('\n', ""));
}
