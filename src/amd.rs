// Get info about AMD graphics cards by parsing the output of rocm-smi.
// This is pretty hacky!  Something better than this is likely needed and hopefully possible.
//
// Note that the returned information is keyed by (device, pid) so that if a process uses multiple
// devices, the total utilization for the process must be summed across devices.  We do this
// to be compatible with the NVIDIA module (nvidia.rs).
//
// I've not been able to combine the two invocations of rocm-smi yet; we have to run the command
// twice.  Not a happy situation.
//
// Also, there is no information here about absolute memory usage numbers.  The cards I have don't
// support getting that information.  Other cards might.  In that case, the --showmemusage switch
// (can be combined with --showgpupids in a single invocation) might be useful.
//
// Even though the output is presented in the same format as for NVIDIA, we only have partial stats
// about the usage of various processes on the various devices.  We divide the utilization of a
// device by the number of processes on the device.  This is approximate.

use crate::command;
use crate::nvidia;
#[cfg(test)]
use crate::util::map;
use std::cmp::Ordering;
use std::collections::HashMap;

pub fn get_amd_information(user_by_pid: &HashMap<String, String>) -> Vec<nvidia::Process> {
    if let Some(concise_raw_text) = command::safe_command(AMD_CONCISE_COMMAND, TIMEOUT_SECONDS) {
        if let Some(showpidgpus_raw_text) =
            command::safe_command(AMD_SHOWPIDGPUS_COMMAND, TIMEOUT_SECONDS)
        {
            extract_amd_information(&concise_raw_text, &showpidgpus_raw_text, user_by_pid)
        } else {
            vec![]
        }
    } else {
        vec![]
    }
}

fn extract_amd_information(
    concise_raw_text: &str,
    showpidgpus_raw_text: &str,
    user_by_pid: &HashMap<String, String>,
) -> Vec<nvidia::Process> {
    let per_device_info = parse_concise_command(concise_raw_text); // device -> (gpu%, mem%)
    let per_pid_info = parse_showpidgpus_command(showpidgpus_raw_text); // pid -> [device, ...]
    let mut num_processes_per_device = vec![];
    num_processes_per_device.resize(per_device_info.len(), 0);
    per_pid_info.iter().for_each(|(_, devs)| {
        devs.iter()
            .for_each(|dev| num_processes_per_device[*dev] += 1)
    });
    let mut processes = vec![];
    // The utilization for one process on one device is the total utilization for the device
    // divided by the number of processes using the device.
    per_pid_info.iter().for_each(|(pid, devs)| {
        devs.iter().for_each(|dev| {
            processes.push(nvidia::Process {
                device: *dev as i32,
                pid: pid.to_string(),
                user: if let Some(u) = user_by_pid.get(&pid.to_string()) {
                    u.to_string()
                } else {
                    "_zombie_".to_owned() + pid
                },
                gpu_pct: per_device_info[*dev].0 / num_processes_per_device[*dev] as f64,
                mem_pct: per_device_info[*dev].1 / num_processes_per_device[*dev] as f64,
                mem_size_kib: 0,
                command: "_noinfo_".to_string(),
            })
        })
    });
    processes.sort_by(|p, q| {
        let fst = p.device.cmp(&q.device);
        if fst == Ordering::Equal {
            p.pid.cmp(&q.pid)
        } else {
            fst
        }
    });
    processes
}

#[cfg(test)]
macro_rules! proc(
    { $a:expr, $b:expr, $c:expr, $d:expr, $e: expr } => {
	nvidia::Process { device: $a,
			  pid: $b.to_string(),
			  user: $c.to_string(),
			  gpu_pct: $d,
			  mem_pct: $e,
			  mem_size_kib: 0,
			  command: "_noinfo_".to_string()
	}
    });

#[test]
fn test_extract_amd_information() {
    let concise = "
================================= Concise Info =================================
GPU  Temp (DieEdge)  AvgPwr  SCLK     MCLK    Fan     Perf  PwrCap  VRAM%  GPU%  
0    53.0c           220.0W  1576Mhz  945Mhz  10.98%  auto  220.0W   57%   99%   
1    26.0c           3.0W    852Mhz   167Mhz  9.41%   auto  220.0W    5%   63%    
================================================================================
";
    let pidgpu = "
============================= GPUs Indexed by PID ==============================
PID 28156 is using 2 DRM device(s):
0 1 
PID 28154 is using 1 DRM device(s):
0 
================================================================================
";
    let users = map! {
    "28156".to_string() => "bob".to_string()
    };
    let zs = extract_amd_information(concise, pidgpu, &users);
    assert!(zs.eq(&vec![
        proc! { 0, "28154", "_zombie_28154", 99.0/2.0, 57.0/2.0 },
        proc! { 0, "28156", "bob", 99.0/2.0, 57.0/2.0 },
        proc! { 1, "28156", "bob", 63.0, 5.0 },
    ]));
}

const TIMEOUT_SECONDS: u64 = 2; // For `rocm-smi`

const AMD_CONCISE_COMMAND: &str = "rocm-smi";

// This returns a vector indexed by device number: (gpu%, mem%)

pub fn parse_concise_command(raw_text: &str) -> Vec<(f64, f64)> {
    let block = find_block(raw_text, "= Concise Info =");
    if block.len() > 1 {
        let hdr = block[0].split_whitespace().collect::<Vec<&str>>();
        if hdr[hdr.len() - 2] == "VRAM%" && hdr[hdr.len() - 1] == "GPU%" {
            let mut i = 1;
            let mut mappings = vec![];
            while i < block.len() {
                let fields = block[i].split_whitespace().collect::<Vec<&str>>();
                let dev = fields[0].parse::<usize>().unwrap_or_default();
                let mem = fields[fields.len() - 2]
                    .strip_suffix('%')
                    .unwrap()
                    .parse::<f64>()
                    .unwrap_or_default();
                let gpu = fields[fields.len() - 1]
                    .strip_suffix('%')
                    .unwrap()
                    .parse::<f64>()
                    .unwrap_or_default();
                if mappings.len() < dev + 1 {
                    mappings.resize(dev + 1, (0.0, 0.0))
                }
                mappings[dev] = (gpu, mem);
                i += 1;
            }
            mappings
        } else {
            vec![]
        }
    } else {
        vec![]
    }
}

#[test]
fn test_parse_concise_command() {
    let xs = parse_concise_command(
        "
================================= Concise Info =================================
GPU  Temp (DieEdge)  AvgPwr  SCLK     MCLK    Fan     Perf  PwrCap  VRAM%  GPU%  
0    53.0c           220.0W  1576Mhz  945Mhz  10.98%  auto  220.0W   57%   99%   
1    26.0c           3.0W    852Mhz   167Mhz  9.41%   auto  220.0W    5%   63%    
================================================================================
",
    );
    assert!(xs.eq(&vec![(99.0, 57.0), (63.0, 5.0)]));
}

const AMD_SHOWPIDGPUS_COMMAND: &str = "rocm-smi --showpidgpus";

// This returns a vector of (PID, DEVICES) where DEVICES is a vector of the devices used
// by the PID.  The PID is a string, the devices are numbers.  See test cases below for
// the various forms expected/supported.
//
// The PIDs are unique, ie, the return value is technically a function.

pub fn parse_showpidgpus_command(raw_text: &str) -> Vec<(&str, Vec<usize>)> {
    let block = find_block(raw_text, "= GPUs Indexed by PID =");
    if block.len() == 1 && block[0].starts_with("No KFD PIDs") {
        // No processes running.
        vec![]
    } else if block.len() > 1 && block.len() % 2 == 0 {
        let mut mappings = vec![];
        let mut i = 0;
        while i < block.len() {
            let xs = block[i].split_whitespace().collect::<Vec<&str>>();
            if xs[0] == "PID" && xs[2] == "is" && xs[3] == "using" && xs[5] == "DRM" {
                let pid = xs[1];
                let numdev = xs[4].parse::<usize>().unwrap_or_default();
                let devices = if numdev > 0 {
                    block[i + 1]
                        .split_whitespace()
                        .map(|d| d.parse::<usize>().unwrap_or_default())
                        .collect::<Vec<usize>>()
                } else {
                    vec![]
                };
                mappings.push((pid, devices))
            }
            i += 2;
        }
        mappings
    } else {
        // Weird output
        vec![]
    }
}

// TODO: Multiple processes on a single device

#[test]
fn test_parse_showpidgpus_command() {
    let xs = parse_showpidgpus_command(
        "
============================= GPUs Indexed by PID ==============================
PID 25774 is using 1 DRM device(s):
0 
================================================================================
",
    );
    assert!(xs.eq(&vec![("25774", vec![0])]));
    let xs = parse_showpidgpus_command(
        "
============================= GPUs Indexed by PID ==============================
No KFD PIDs currently running
================================================================================
",
    );
    assert!(xs.eq(&vec![]));

    let xs = parse_showpidgpus_command(
        "
============================= GPUs Indexed by PID ==============================
PID 28156 is using 1 DRM device(s):
1 
PID 28154 is using 1 DRM device(s):
0 
================================================================================
",
    );
    assert!(xs.eq(&vec![("28156", vec![1]), ("28154", vec![0])]));
    let xs = parse_showpidgpus_command(
        "
============================= GPUs Indexed by PID ==============================
PID 29212 is using 2 DRM device(s):
0 1 
================================================================================
",
    );
    assert!(xs.eq(&vec![("29212", vec![0, 1])]));
}

// Grab the first block of rocm-smi output we see that contains the trigger string, and return the
// lines within that block.

fn find_block<'a>(raw_text: &'a str, trigger: &str) -> Vec<&'a str> {
    let lines = raw_text.lines().collect::<Vec<&str>>();
    let mut i = 0;
    let mut b = vec![];
    while i < lines.len() && !lines[i].contains(trigger) {
        i += 1;
    }
    if i < lines.len() && lines[i].contains(trigger) {
        i += 1;
        while i < lines.len() && !is_terminator(lines[i]) {
            b.push(lines[i]);
            i += 1;
        }
    }
    b
}

fn is_terminator(s: &str) -> bool {
    s.chars().all(|c| c == '=')
}

#[test]
fn test_find_block() {
    assert!(find_block(
        "
============================= xGPUs Indexed by PID ==============================
============================= GPUs Indexed by PID ==============================
PID 25774 is using 1 DRM device(s):
0 
================================================================================
",
        "= GPUs Indexed by PID ="
    )
    .eq(&vec!["PID 25774 is using 1 DRM device(s):", "0 "]))
}
