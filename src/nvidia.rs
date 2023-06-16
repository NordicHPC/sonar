// Run nvidia-smi and return a vector of process information.

use crate::command;
use crate::util;
use std::collections::HashMap;
#[cfg(test)]
use crate::util::map;

pub struct Process {
    pub device: i32,		// -1 for "unknown", otherwise 0..num_devices-1
    pub pid: String,		// Process ID
    pub user: String,		// User name, _zombie_PID for zombies
    pub gpu_pct: f64,		// Percent of GPU, 0.0 for zombies
    pub mem_pct: f64,		// Percent of memory, 0.0 for zombies
    pub mem_size_kib: usize,	// Memory use in KiB, _not_ zero for zombies
    pub command: String		// The command, _unknown_ for zombies
}

pub fn get_nvidia_information(timeout_seconds: u64, user_by_pid: &HashMap<String, String>) -> Vec<Process> {
    if let Some(pmon_raw_text) = command::safe_command(NVIDIA_PMON_COMMAND, timeout_seconds) {
	let mut processes = parse_pmon_output(&pmon_raw_text, user_by_pid);
        if let Some(query_raw_text) = command::safe_command(NVIDIA_QUERY_COMMAND, timeout_seconds) {
	    processes.append(&mut parse_query_output(&query_raw_text, user_by_pid));
	}
	processes
    } else {
	vec![]
    }
}

// For prototyping purposes (and maybe it's good enough for production?), parse the output of
// `nvidia-smi pmon`.  This output has a couple of problems:
//
//  - it is (documented to be) not necessarily stable
//  - it does not orphaned processes holding onto GPU memory, the way nvtop can do
//
// To fix the latter problem we do something with --query-compute-apps, see later.
//
// TODO: We could consider using the underlying C library instead, but this adds a fair
// amount of complexity.  See the nvidia-smi manual page.

const NVIDIA_PMON_COMMAND: &str = "nvidia-smi pmon -c 1 -s mu";

// Returns (user-name, pid, command-name) -> (device-mask, gpu-util-pct, gpu-mem-pct, gpu-mem-size-in-kib)
// where the values are summed across all devices and the device-mask is a bitmask for the
// GPU devices used by that process.  For a system with 8 cards, utilization
// can reach 800% and the memory size can reach the sum of the memories on the cards.

fn parse_pmon_output(raw_text: &str, user_by_pid: &HashMap<String, String>) -> Vec<Process> {
    raw_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let (_start_indices, parts) = util::chunks(line);
            let device = parts[0].parse::<i32>().unwrap();
            let pid = parts[1];
            let mem_size = parts[3].parse::<usize>().unwrap_or(0);
            let gpu_pct = parts[4].parse::<f64>().unwrap_or(0.0);
            let mem_pct = parts[5].parse::<f64>().unwrap_or(0.0);
            // For nvidia-smi, we use the first word because the command produces blank-padded
            // output.  We can maybe do better by considering non-empty words.
            let command = parts[8].to_string();
            let user = match user_by_pid.get(pid) {
                Some(name) => name.clone(),
                None => "_zombie_".to_owned() + pid,
            };
            (
                pid,
                device,
                user,
                mem_size,
                gpu_pct,
                mem_pct,
                command,
            )
        })
        .filter(|(pid, ..)| *pid != "-")
        .map(|(pid, device, user, mem_size, gpu_pct, mem_pct, command)| {
	    Process {
		device,
		pid: pid.to_string(),
		user,
		gpu_pct,
		mem_pct,
		mem_size_kib: mem_size,
		command }})
	.collect::<Vec<Process>>()
}

// We use this to get information about processes that are not captured by pmon.  It's hacky
// but it works.

const NVIDIA_QUERY_COMMAND: &str =
    "nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits";

// Same signature as extract_nvidia_pmon_processes(), q.v. but user is always "_zombie_" and command
// is always "_unknown_".  Only pids not in user_by_pid are returned.

fn parse_query_output(raw_text: &str, user_by_pid: &HashMap<String, String>) -> Vec<Process> {
    raw_text
        .lines()
        .map(|line| {
            let (_start_indices, parts) = util::chunks(line);
            let pid = parts[0].strip_suffix(',').unwrap();
            let mem_usage = parts[1].parse::<usize>().unwrap();
            let user = "_zombie_".to_owned() + pid;
            let command = "_unknown_";
            ( pid.to_string(), user, command.to_string(), mem_usage * 1024 )
        })
        .filter(|(pid, ..)| !user_by_pid.contains_key(pid))
	.map(|(pid, user, command, mem_size_kib)| {
	    Process {
		device: !0,
		pid,
		user,
		gpu_pct: 0.0,
		mem_pct: 0.0,
		mem_size_kib,
		command }})
	.collect::<Vec<Process>>()
}
    
// Shared test cases for the NVIDIA stuff
/*
#[cfg(test)]
mod test_nvidia {
    use super::*;

    fn mkusers() -> HashMap<String, String> {
        map! {
            "447153".to_string() => "bob".to_string(),
            "447160".to_string() => "bob".to_string(),
            "1864615".to_string() => "alice".to_string(),
            "2233095".to_string() => "charlie".to_string(),
            "2233469".to_string() => "charlie".to_string()
        }
    }

    // $ nvidia-smi pmon -c 1 -s mu
    #[test]
    fn test_extract_nvidia_pmon_processes() {
        let text = "# gpu        pid  type    sm   mem   enc   dec   command
# Idx          #   C/G     %     %     %     %   name
# gpu        pid  type    fb    sm   mem   enc   dec   command
# Idx          #   C/G    MB     %     %     %     %   name
    0     447153     C  7669     -     -     -     -   python3.9      
    0     447160     C 11057     -     -     -     -   python3.9      
    0     506826     C 11057     -     -     -     -   python3.9      
    0    1864615     C  1635    40     0     -     -   python         
    1    1864615     C   535     -     -     -     -   python         
    1    2233095     C 24395    84    23     -     -   python3        
    2    1864615     C   535     -     -     -     -   python         
    2    1448150     C  9383     -     -     -     -   python3        
    3    1864615     C   535     -     -     -     -   python         
    3    2233469     C 15771    90    23     -     -   python3        
";
        let processes = extract_nvidia_pmon_processes(text, &mkusers());
        assert!(
            processes
                == map! {
                    ("bob".to_string(), "447153".to_string(), "python3.9".to_string()) =>      (0b1, 0.0, 0.0, 7669*1024),
                    ("bob".to_string(), "447160".to_string(), "python3.9".to_string()) =>      (0b1, 0.0, 0.0, 11057*1024),
                    ("_zombie_506826".to_string(), "506826".to_string(), "python3.9".to_string()) => (0b1, 0.0, 0.0, 11057*1024),
                    ("alice".to_string(), "1864615".to_string(), "python".to_string()) =>      (0b1111, 40.0, 0.0, (1635+535+535+535)*1024),
                    ("charlie".to_string(), "2233095".to_string(), "python3".to_string()) =>   (0b10, 84.0, 23.0, 24395*1024),
                    ("_zombie_1448150".to_string(), "1448150".to_string(), "python3".to_string()) =>  (0b100, 0.0, 0.0, 9383*1024),
                    ("charlie".to_string(), "2233469".to_string(), "python3".to_string()) =>   (0b1000, 90.0, 23.0, 15771*1024)
                }
        );
    }

    // $ nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits
    #[test]
    fn test_extract_nvidia_query_processes() {
        let text = "2233095, 1190
3079002, 2350
1864615, 1426";
        let processes = extract_nvidia_query_processes(text, &mkusers());
        assert!(
            processes
                == map! {
                    ("_zombie_3079002".to_string(), "3079002".to_string(), "_unknown_".to_string()) => (!0, 0.0, 0.0, 2350*1024)
                }
        );
    }
}
*/
