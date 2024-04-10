/// Run nvidia-smi and return a vector of process samples.
///
/// The information is keyed by (device, pid) so that if a process uses multiple devices, the total
/// utilization for the process must be summed across devices.  (This is the natural mode of output
/// for `nvidia-smi pmon`.)
///
/// Crucially, the data are sampling data: they contain no (long) running averages, but are
/// snapshots of the system at the time the sample is taken.
use crate::command::{self, CmdError};
use crate::gpu;
use crate::ps::UserTable;
use crate::util;
use crate::TIMEOUT_SECONDS;

#[cfg(test)]
use crate::util::map;

// `nvidia-smi -a` dumps a lot of information about all the cards in a semi-structured form,
// each line a textual keyword/value pair.
//
// "Product Name" names the card.  Following the string "FB Memory Usage", "Total" has the
// memory of the card.
//
// Parsing all the output lines in order yields the information about all the cards.

pub fn get_nvidia_configuration() -> Option<Vec<gpu::Card>> {
    match command::safe_command("nvidia-smi", &["-a"], TIMEOUT_SECONDS) {
        Ok(raw_text) => {
            let mut cards = vec![];
            let mut looking_for_total = false;
            let mut model_name = None;
            for l in raw_text.lines() {
                // The regular expressions that trigger state transitions are really these:
                //
                //   /^\s*Product Name\s*:\s*(.*)$/
                //   /^\s*FB Memory Usage\s*$/
                //   /^\s*Total\s*:\s*(\d+)\s*MiB\s*$/
                //
                // but we simplify a bit and use primitive string manipulation.
                let l = l.trim();
                if looking_for_total {
                    if l.starts_with("Total") && l.ends_with("MiB") {
                        if let Some((_, after)) = l.split_once(':') {
                            let rest = after.strip_suffix("MiB").unwrap().trim();
                            if let Ok(n) = rest.parse::<i64>() {
                                if let Some(m) = model_name {
                                    cards.push(gpu::Card {
                                        model: m,
                                        mem_size_kib: n * 1024,
                                    });
                                    model_name = None;
                                }
                            }
                        }
                    }
                } else {
                    if l.starts_with("Product Name") {
                        if let Some((_, rest)) = l.split_once(':') {
                            model_name = Some(rest.trim().to_string());
                            continue;
                        }
                    }
                    if l.starts_with("FB Memory Usage") {
                        looking_for_total = true;
                        continue;
                    }
                }
                looking_for_total = false;
            }
            Some(cards)
        }
        Err(_) => None,
    }
}

// Err(e) really means the command started running but failed, for the reason given.  If the
// command could not be found, we return Ok(vec![]).

pub fn get_nvidia_information(user_by_pid: &UserTable) -> Result<Vec<gpu::Process>, String> {
    match command::safe_command(NVIDIA_PMON_COMMAND, NVIDIA_PMON_ARGS, TIMEOUT_SECONDS) {
        Ok(pmon_raw_text) => {
            let mut processes = parse_pmon_output(&pmon_raw_text, user_by_pid);
            match command::safe_command(NVIDIA_QUERY_COMMAND, NVIDIA_QUERY_ARGS, TIMEOUT_SECONDS) {
                Ok(query_raw_text) => {
                    processes.append(&mut parse_query_output(&query_raw_text, user_by_pid));
                    Ok(processes)
                }
                Err(e) => Err(format!("{:?}", e)),
            }
        }
        Err(CmdError::CouldNotStart(_)) => Ok(vec![]),
        Err(e) => Err(format!("{:?}", e)),
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
// Note that `-c 1 -s u` gives us more or less instantaneous utilization, not some long-running
// average.
//
// TODO: We could consider using the underlying C library instead, but this adds a fair
// amount of complexity.  See the nvidia-smi manual page.

const NVIDIA_PMON_COMMAND: &str = "nvidia-smi";
const NVIDIA_PMON_ARGS: &[&str] = &["pmon", "-c", "1", "-s", "mu"];

// Returns (user-name, pid, command-name) -> (device-mask, gpu-util-pct, gpu-mem-pct, gpu-mem-size-in-kib)
// where the values are summed across all devices and the device-mask is a bitmask for the
// GPU devices used by that process.  For a system with 8 cards, utilization
// can reach 800% and the memory size can reach the sum of the memories on the cards.

fn parse_pmon_output(raw_text: &str, user_by_pid: &UserTable) -> Vec<gpu::Process> {
    raw_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let (_start_indices, parts) = util::chunks(line);
            let device = parts[0].parse::<usize>().unwrap();
            let pid = parts[1];
            let mem_size = parts[3].parse::<usize>().unwrap_or(0);
            let gpu_pct = parts[4].parse::<f64>().unwrap_or(0.0);
            let mem_pct = parts[5].parse::<f64>().unwrap_or(0.0);
            // For nvidia-smi, we use the first word because the command produces blank-padded
            // output.  We can maybe do better by considering non-empty words.
            let command = parts[8].to_string();
            (pid, device, mem_size, gpu_pct, mem_pct, command)
        })
        .filter(|(pid, ..)| *pid != "-")
        .map(|(pid_str, device, mem_size, gpu_pct, mem_pct, command)| {
            let pid = pid_str.parse::<usize>().unwrap();
            let user = match user_by_pid.get(&pid) {
                Some((name, uid)) => (name.to_string(), *uid),
                None => ("_zombie_".to_owned() + pid_str, gpu::ZOMBIE_UID),
            };
            gpu::Process {
                device: Some(device),
                pid,
                user: user.0,
                uid: user.1,
                gpu_pct,
                mem_pct,
                mem_size_kib: mem_size * 1024,
                command,
            }
        })
        .collect::<Vec<gpu::Process>>()
}

// We use this to get information about processes that are not captured by pmon.  It's hacky
// but it works.

const NVIDIA_QUERY_COMMAND: &str = "nvidia-smi";

const NVIDIA_QUERY_ARGS: &[&str] =
    &["--query-compute-apps=pid,used_memory", "--format=csv,noheader,nounits"];

// Same signature as extract_nvidia_pmon_processes(), q.v. but user is always "_zombie_" and command
// is always "_unknown_".  Only pids not in user_by_pid are returned.

fn parse_query_output(raw_text: &str, user_by_pid: &UserTable) -> Vec<gpu::Process> {
    raw_text
        .lines()
        .map(|line| {
            let (_start_indices, parts) = util::chunks(line);
            let pid_str = parts[0].strip_suffix(',').unwrap();
            let pid = pid_str.parse::<usize>().unwrap();
            let mem_usage = parts[1].parse::<usize>().unwrap();
            let user = "_zombie_".to_owned() + pid_str;
            let command = "_unknown_";
            (pid, user, command.to_string(), mem_usage * 1024)
        })
        .filter(|(pid, ..)| !user_by_pid.contains_key(pid))
        .map(|(pid, user, command, mem_size_kib)| gpu::Process {
            device: None,
            pid,
            user,
            uid: gpu::ZOMBIE_UID,
            gpu_pct: 0.0,
            mem_pct: 0.0,
            mem_size_kib,
            command,
        })
        .collect::<Vec<gpu::Process>>()
}

#[cfg(test)]
fn mkusers() -> UserTable<'static> {
    map! {
        447153 => ("bob", 1001),
        447160 => ("bob", 1001),
        1864615 => ("alice", 1002),
        2233095 => ("charlie", 1003),
        2233469 => ("charlie", 1003)
    }
}

#[cfg(test)]
pub fn parsed_pmon_output() -> Vec<gpu::Process> {
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
    parse_pmon_output(text, &mkusers())
}

#[cfg(test)]
macro_rules! proc(
    { $a:expr, $b:expr, $c:expr, $d:expr, $e: expr, $f:expr, $g:expr, $h:expr } => {
        gpu::Process { device: $a,
                       pid: $b,
                       user: $c.to_string(),
                       uid: $d,
                       gpu_pct: $e,
                       mem_pct: $f,
                       mem_size_kib: $g,
                       command: $h.to_string()
        }
    });

#[test]
fn test_parse_pmon_output() {
    assert!(parsed_pmon_output().into_iter().eq(vec![
        proc! { Some(0),  447153, "bob",            1001, 0.0,  0.0,  7669 * 1024, "python3.9" },
        proc! { Some(0),  447160, "bob",            1001, 0.0,  0.0, 11057 * 1024, "python3.9" },
        proc! { Some(0),  506826, "_zombie_506826", gpu::ZOMBIE_UID, 0.0,  0.0, 11057 * 1024, "python3.9" },
        proc! { Some(0), 1864615, "alice",          1002, 40.0,  0.0,  1635 * 1024, "python" },
        proc! { Some(1), 1864615, "alice",          1002,  0.0,  0.0,   535 * 1024, "python" },
        proc! { Some(1), 2233095, "charlie",        1003, 84.0, 23.0, 24395 * 1024, "python3" },
        proc! { Some(2), 1864615, "alice",          1002, 0.0,  0.0,   535 * 1024, "python" },
        proc! { Some(2), 1448150, "_zombie_1448150", gpu::ZOMBIE_UID, 0.0,  0.0,  9383 * 1024, "python3"},
        proc! { Some(3), 1864615, "alice",          1002,  0.0,  0.0,   535 * 1024, "python" },
        proc! { Some(3), 2233469, "charlie",        1003, 90.0, 23.0, 15771 * 1024, "python3" }
    ]))
}

#[cfg(test)]
pub fn parsed_query_output() -> Vec<gpu::Process> {
    let text = "2233095, 1190
3079002, 2350
1864615, 1426";
    parse_query_output(text, &mkusers())
}

#[test]
fn test_parse_query_output() {
    assert!(parsed_query_output().into_iter().eq(vec![
        proc! { None, 3079002, "_zombie_3079002", gpu::ZOMBIE_UID, 0.0, 0.0, 2350 * 1024, "_unknown_" }
    ]))
}
