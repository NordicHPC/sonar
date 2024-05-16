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
            let mut processes = parse_pmon_output(&pmon_raw_text, user_by_pid)?;
            match command::safe_command(NVIDIA_QUERY_COMMAND, NVIDIA_QUERY_ARGS, TIMEOUT_SECONDS) {
                Ok(query_raw_text) => {
                    processes.append(&mut parse_query_output(&query_raw_text, user_by_pid)?);
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
// To fix the former (in part), we parse the line that starts with '# gpu' to get field name
// indices, and then use those indices to fetch data.
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
// where the values are summed across all devices and the device-mask is a bitmask for the GPU
// devices used by that process.  For a system with 8 cards, utilization can reach 800% and the
// memory size can reach the sum of the memories on the cards.

fn parse_pmon_output(raw_text: &str, user_by_pid: &UserTable) -> Result<Vec<gpu::Process>, String> {
    let mut device_index = None;
    let mut pid_index = None;
    let mut mem_size_index = None;
    let mut gpu_util_index = None;
    let mut mem_util_index = None;
    let mut command_index = None;
    let mut processes = vec![];
    for line in raw_text.lines() {
        if line.starts_with("# gpu") {
            let header_parts = util::chunks(line).1;
            assert!(header_parts[0] == "#");
            assert!(header_parts[1] == "gpu");
            if device_index.is_some() {
                return Err("Duplicate header line in pmon output".to_string())
            }
            device_index = Some(0);
            for (i, p) in header_parts.iter().enumerate() {
                match *p {
                    "pid" => {
                        if pid_index.is_some() {
                            return Err("Duplicate pid index".to_string())
                        }
                        pid_index = Some(i-1)
                    }
                    "fb" => {
                        if mem_size_index.is_some() {
                            return Err("Duplicate fb index".to_string())
                        }
                        mem_size_index = Some(i-1)
                    }
                    "sm" => {
                        if gpu_util_index.is_some() {
                            return Err("Duplicate sm index".to_string())
                        }
                        gpu_util_index = Some(i-1)
                    }
                    "mem" => {
                        if mem_util_index.is_some() {
                            return Err("Duplicate mem index".to_string())
                        }
                        mem_util_index = Some(i-1)
                    }
                    "command" => {
                        if command_index.is_some() {
                            return Err("Duplicate command index".to_string())
                        }
                        command_index = Some(i-1)
                    }
                    _ => {}
                }
            }
            // Require all indices to come from the same header.
            if device_index.is_none() || pid_index.is_none() || mem_size_index.is_none() ||
                gpu_util_index.is_none() || mem_util_index.is_none() || command_index.is_none() {
                return Err("Missing required field in pmon output".to_string())
            }
            continue
        }
        if line.starts_with('#') {
            continue
        }
        if device_index.is_none() {
            return Err("Missing header in pmon output".to_string())
        }
        let parts = util::chunks(line).1;
        let pid_str = parts[pid_index.unwrap()];
        let device_str = parts[device_index.unwrap()];
        if pid_str == "-" || device_str == "-" {
            // This is definitely not an error but we can't use the data
            continue
        }
        let pid = match pid_str.parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                // It's possible that this ought to be reported as an error
                continue
            }
        };
        let device = match device_str.parse::<usize>() {
            Ok(n) => Some(n),
            Err(_) => {
                // It's possible that this ought to be reported as an error
                continue
            }
        };
        // The following parsers are allowed to fail quietly as missing information may be
        // represented simply as "-", and we're OK with that.
        let mem_size = parts[mem_size_index.unwrap()].parse::<usize>().unwrap_or(0);
        let gpu_util_pct = parts[gpu_util_index.unwrap()].parse::<f64>().unwrap_or(0.0);
        let mem_util_pct = parts[mem_util_index.unwrap()].parse::<f64>().unwrap_or(0.0);
        // For nvidia-smi, we use the first word because the command produces blank-padded
        // output.  We can maybe do better by considering non-empty words.
        let command = parts[command_index.unwrap()].to_string();
        let user = match user_by_pid.get(&pid) {
            Some((name, uid)) => (name.to_string(), *uid),
            None => ("_zombie_".to_owned() + pid_str, gpu::ZOMBIE_UID),
        };
        processes.push(gpu::Process {
            device,
            pid,
            user: user.0,
            uid: user.1,
            gpu_pct: gpu_util_pct,
            mem_pct: mem_util_pct,
            mem_size_kib: mem_size * 1024,
            command,
        });
    }
    if device_index.is_none() {
        return Err("Missing header line in pmon output".to_string())
    }
    Ok(processes)
}

// We use this to get information about processes that are not captured by pmon.  It's hacky
// but it works.

const NVIDIA_QUERY_COMMAND: &str = "nvidia-smi";

const NVIDIA_QUERY_ARGS: &[&str] = &[
    "--query-compute-apps=pid,used_memory",
    "--format=csv,noheader,nounits",
];

// Same signature as extract_nvidia_pmon_processes(), q.v. but user is always "_zombie_<PID>" and
// command is always "_unknown_".  Only pids not in user_by_pid are returned.
//
// This parser does not have quite the same resilience features as the pmon parser above because we
// ask for a specific output format and specific fields, and we will assume that the request is
// being honored.  Still, that format looks a little brittle: it's supposed to be "CSV" but fields
// are separated not by "," but by ",<SPACE>".  There's a risk this could change.  So there's some
// checking here.

fn parse_query_output(raw_text: &str, user_by_pid: &UserTable) -> Result<Vec<gpu::Process>, String> {
    let mut result = vec![];
    for line in raw_text.lines() {
        let (_start_indices, parts) = util::chunks(line);
        if parts.len() != 2 {
            return Err("Unexpected output from nvidia-smi query: too many fields".to_string())
        }
        let pid_str = parts[0].trim_end_matches(',');
        let pid = match pid_str.parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                return Err("Unexpected output from nvidia-smi query: first field is not pid".to_string())
            }
        };
        let mem_usage = match parts[1].parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                return Err("Unexpected output from nvidia-smi query: second field is not memory size".to_string())
            }
        };
        // Do this after parsing to get some sensible syntax checking
        if user_by_pid.contains_key(&pid) {
            continue
        }
        let user = "_zombie_".to_owned() + pid_str;
        let command = "_unknown_";
        let mem_size_kib = mem_usage * 1024;
        result.push(gpu::Process {
            device: None,
            pid,
            user,
            uid: gpu::ZOMBIE_UID,
            gpu_pct: 0.0,
            mem_pct: 0.0,
            mem_size_kib,
            command: command.to_string(),
        })
    }
    Ok(result)
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
pub fn parsed_pmon_545_output() -> Vec<gpu::Process> {
    let text = "# gpu        pid  type    fb    sm   mem   enc   dec   command
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
    parse_pmon_output(text, &mkusers()).unwrap()
}

// 550 added a number of columns, these are from gpu-13.fox (new are ccpm, jpg, ofa).  The data are
// the same synthetic data as above, we just have new blank columns.
#[cfg(test)]
pub fn parsed_pmon_550_output() -> Vec<gpu::Process> {
    let text = "# gpu        pid  type    fb    ccpm  sm   mem   enc   dec   jpg   ofa   command
# Idx          #   C/G    MB     MB      %     %     %     %     %      %    name
    0     447153     C  7669     -     -     -     -     -     -     -   python3.9
    0     447160     C 11057     -     -     -     -     -     -     -   python3.9
    0     506826     C 11057     -     -     -     -     -     -     -   python3.9
    0    1864615     C  1635    -     40     0     -     -     -     -   python
    1    1864615     C   535    -      -     -     -     -     -     -   python
    1    2233095     C 24395    -     84    23     -     -     -     -   python3
    2    1864615     C   535    -      -     -     -     -     -     -   python
    2    1448150     C  9383    -      -     -     -     -     -     -   python3
    3    1864615     C   535    -      -     -     -     -     -     -   python
    3    2233469     C 15771    -     90    23     -     -     -     -   python3
";
    parse_pmon_output(text, &mkusers()).unwrap()
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
fn test_parse_pmon_545_output() {
    let expected = vec![
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
    ];
    let actual = parsed_pmon_545_output();
    assert!(expected.eq(&actual));
}

#[test]
fn test_parse_pmon_550_output() {
    let expected = vec![
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
    ];
    let actual = parsed_pmon_550_output();
    assert!(expected.eq(&actual));
}

#[test]
fn test_no_pmon_header1() {
    let text = "# xpu        pid  type    fb    sm   mem   enc   dec   command
# Idx          #   C/G    MB     %     %     %     %   name
    0     447153     C  7669     -     -     -     -   python3.9
";
    assert!(parse_pmon_output(text, &mkusers()).is_err());
}

#[test]
fn test_no_pmon_header2() {
    let text = "# xpu        pid  type    fb    sm   mem   enc   dec   command
# Idx          #   C/G    MB     %     %     %     %   name
";
    assert!(parse_pmon_output(text, &mkusers()).is_err());
}

#[test]
fn test_duplicate_pmon_header() {
    let text = "# gpu        pid  type    fb    sm   mem   enc   dec   command
# Idx          #   C/G    MB     %     %     %     %   name
# gpu        pid  type    fb    sm   mem   enc   dec   command
    0     447153     C  7669     -     -     -     -   python3.9
";
    assert!(parse_pmon_output(text, &mkusers()).is_err());
}

// We could repeat the following two for all the fields but this at least tests the logic.

#[test]
fn test_no_fb_index() {
    let text = "# gpu        pid  type    xb    sm   mem   enc   dec   command
# Idx          #   C/G    MB     %     %     %     %   name
    0     447153     C  7669     -     -     -     -   python3.9
";
    assert!(parse_pmon_output(text, &mkusers()).is_err());
}

#[test]
fn test_dup_fb_index() {
    let text = "# gpu        pid  type    fb    sm   mem   enc   fb   command
# Idx          #   C/G    MB     %     %     %     %   name
    0     447153     C  7669     -     -     -     -   python3.9
";
    assert!(parse_pmon_output(text, &mkusers()).is_err());
}

#[cfg(test)]
pub fn parsed_query_output() -> Vec<gpu::Process> {
    let text = "2233095, 1190
3079002, 2350
1864615, 1426";
    parse_query_output(text, &mkusers()).unwrap()
}

#[test]
fn test_parse_query_output() {
    assert!(parsed_query_output().into_iter().eq(vec![
        proc! { None, 3079002, "_zombie_3079002", gpu::ZOMBIE_UID, 0.0, 0.0, 2350 * 1024, "_unknown_" }
    ]))
}

// Fields not properly space-separated
#[test]
fn test_parsed_bad_query_output1() {
    let text = "2233095,1190
3079002, 2350
1864615, 1426";
    assert!(parse_query_output(text, &mkusers()).is_err());
}

// Too few fields (there is a trailing space on line 2)
#[test]
fn test_parsed_bad_query_output2() {
    let text = "2233095, 1190
3079002, 
1864615, 1426";
    assert!(parse_query_output(text, &mkusers()).is_err());
}

// Too many fields
#[test]
fn test_parsed_bad_query_output3() {
    let text = "2233095, 1190
3079002, 1, 2
1864615, 1426";
    assert!(parse_query_output(text, &mkusers()).is_err());
}

// Non-integer field
#[test]
fn test_parsed_bad_query_output4() {
    let text = "2233095, 1190
3079002, 1
1864615x, 1426";
    assert!(parse_query_output(text, &mkusers()).is_err());
}

// Non-integer field
#[test]
fn test_parsed_bad_query_output5() {
    let text = "2233095, 1190
3079002, 1
1864615, y1426";
    assert!(parse_query_output(text, &mkusers()).is_err());
}

