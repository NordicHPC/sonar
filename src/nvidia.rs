// Get info about Nvidia graphics cards by parsing the output of nvidia-smi.

use crate::command::{self, CmdError};
use crate::gpu;
use crate::ps::UserTable;
use crate::util;
use crate::TIMEOUT_SECONDS;

#[cfg(test)]
use crate::util::map;
use std::path::Path;

pub struct NvidiaGPU {
    // At the moment, all this information is the result of a single run of nvidia-smi, so we cache
    // it since there will otherwise be two runs.
    //
    // TODO: It's possible the process information should be derived from this run, too.
    info: Option<Result<Vec<PerCard>, String>>,
}

#[derive(Default)]
struct PerCard {
    info: gpu::Card,
    state: gpu::CardState,
}

pub fn probe() -> Option<Box<dyn gpu::GPU>> {
    if nvidia_present() {
        Some(Box::new(NvidiaGPU { info: None }))
    } else {
        None
    }
}

impl gpu::GPU for NvidiaGPU {
    fn get_manufacturer(&mut self) -> String {
        "NVIDIA".to_string()
    }

    fn get_card_configuration(&mut self) -> Result<Vec<gpu::Card>, String> {
        if self.info.is_none() {
            self.info = Some(get_nvidia_configuration(&vec!["-a"]))
        }
        match self.info.as_ref().unwrap() {
            Ok(data) => Ok(data
                .iter()
                .map(|pc| pc.info.clone())
                .collect::<Vec<gpu::Card>>()),
            Err(e) => Err(e.clone()),
        }
    }

    fn get_process_utilization(
        &mut self,
        user_by_pid: &UserTable,
    ) -> Result<Vec<gpu::Process>, String> {
        get_nvidia_utilization(user_by_pid)
    }

    fn get_card_utilization(&mut self) -> Result<Vec<gpu::CardState>, String> {
        if self.info.is_none() {
            self.info = Some(get_nvidia_configuration(&vec!["-a"]))
        }
        match self.info.as_ref().unwrap() {
            Ok(data) => Ok(data
                .iter()
                .map(|pc| pc.state.clone())
                .collect::<Vec<gpu::CardState>>()),
            Err(e) => Err(e.clone()),
        }
    }
}

// On all nodes we've looked at (Fox, Betzy, ML systems), /sys/module/nvidia exists iff there are
// nvidia accelerators present.

fn nvidia_present() -> bool {
    return Path::new("/sys/module/nvidia").exists();
}

// `nvidia-smi -a` (aka `nvidia-smi -q`) dumps a lot of information about all the cards in a
// semi-structured form.  Without additional arguments it is fairly slow, the reason being it also
// obtains information about running processes.  But if we only run it without more arguments for
// sysinfo then that's OK.  For other purposes, adding -d <SELECTOR>,... is helpful for performance.
//
// In brief, the input is a set of lines with a preamble followed by zero or more cards.
// Indentation indicates nesting of sections and subsections.  Everything ends implicitly; if an
// indent-4 line is encountered inside a section then that ends the section, if an indent-0 line is
// encountered inside a section or card then that ends the card.
//
// Against that background, these regexes matching full lines describe a state machine:
//
// - a line matching /^CUDA Version\s*:\s*(.*)$/ registers the common CUDA version
// - a line matching /^Driver Version\s*:\s*(.*)$/ registers the common driver version
// - a line matching /^GPU (.*)/ starts a new card, the card is named by $1.
// - a line matching /^\s{4}(${name})\s*:\s*(.*)$/ names a keyword-value pair not in a section
//   where $1 is the keyword and $2 is the value; ${name} is /[A-Z][^:]*/
// - a line matching /^\s{4}(${name})$/ is the start of a top-level section
//   a line matching /^\s{8}(${name})\s*:\s*(.*)$/ names a keyword-value pair in a section,
//   where $1 is the keyword and $2 is the value
// - a line matching /^\s+(.*)$/ but not any of the above is either a subsubsection value,
//   a subsubsection start, or other gunk we don't care about
// - a blank line or eof marks the end of the card
//
// To avoid building a lexer/parser or playing with regexes we can match against the entire line or
// the beginning of line, within a context.  Note the use of "==" rather than "starts_with" to enter
// into subsections is deliberate, as several subsections may start with the same word ("Clocks").
//
// It looks like nvidia-smi enumerates cards in a consistent order by increasing bus address, so
// just take that to be the card index.  (In contrast, the Minor Number does not always follow that
// order.)

fn get_nvidia_configuration(smi_args: &[&str]) -> Result<Vec<PerCard>, String> {
    match command::safe_command("nvidia-smi", smi_args, TIMEOUT_SECONDS) {
        Ok(raw_text) => Ok(parse_nvidia_configuration(&raw_text)),
        Err(CmdError::CouldNotStart(_)) => Ok(vec![]),
        Err(e) => Err(format!("{:?}", e)),
    }
}

fn parse_nvidia_configuration(raw_text: &str) -> Vec<PerCard> {
    enum State {
        Preamble,
        InCard,
        FbMemoryUsage,
        GpuPowerReadings,
        MaxClocks,
        Clocks,
        Utilization,
        Temperature,
    }
    let mut cuda = "".to_string();
    let mut driver = "".to_string();
    let mut state = State::Preamble;
    let mut cards = vec![];
    let mut card: PerCard = Default::default();
    'next_line: for l in raw_text.lines() {
        'reprocess_line: loop {
            match state {
                State::Preamble => {
                    if l.starts_with("CUDA Version") {
                        cuda = field_value(l);
                    } else if l.starts_with("Driver Version") {
                        driver = field_value(l);
                    } else if l.starts_with("GPU ") {
                        if !card.info.bus_addr.is_empty() {
                            cards.push(card);
                        }
                        card = Default::default();
                        card.info.bus_addr = l[4..].to_string();
                        card.info.driver = driver.clone();
                        card.info.firmware = cuda.clone();
                        card.info.index = cards.len() as i32;
                        card.state.index = card.info.index;
                        state = State::InCard;
                    }
                    continue 'next_line;
                }
                State::InCard => {
                    if !l.starts_with("    ") {
                        state = State::Preamble;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("    Product Name") {
                        card.info.model = field_value(l);
                    } else if l.starts_with("    Product Architecture") {
                        card.info.arch = field_value(l);
                    } else if l.starts_with("    GPU UUID") {
                        card.info.uuid = field_value(l);
                    } else if l.starts_with("    Fan Speed") {
                        if let Ok(n) = field_value_stripped(l, "%").parse::<f32>() {
                            card.state.fan_speed_pct = n;
                        }
                    } else if l.starts_with("    Compute Mode") {
                        card.state.compute_mode = field_value(l);
                    } else if l.starts_with("    Performance State") {
                        card.state.perf_state = field_value(l);
                    } else if l == "    FB Memory Usage" {
                        state = State::FbMemoryUsage;
                    } else if l == "    GPU Power Readings" {
                        state = State::GpuPowerReadings;
                    } else if l == "    Max Clocks" {
                        state = State::MaxClocks;
                    } else if l == "    Clocks" {
                        state = State::Clocks;
                    } else if l == "    Utilization" {
                        state = State::Utilization;
                    } else if l == "    Temperature" {
                        state = State::Temperature;
                    }
                    continue 'next_line;
                }
                State::FbMemoryUsage => {
                    if !l.starts_with("        ") {
                        state = State::InCard;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("        Total") {
                        if let Ok(n) = field_value_stripped(l, "MiB").parse::<i64>() {
                            card.info.mem_size_kib = n * 1024;
                        }
                    } else if l.starts_with("        Reserved") {
                        if let Ok(n) = field_value_stripped(l, "MiB").parse::<i64>() {
                            card.state.mem_reserved_kib = n * 1024;
                        }
                    } else if l.starts_with("        Used") {
                        if let Ok(n) = field_value_stripped(l, "MiB").parse::<i64>() {
                            card.state.mem_used_kib = n * 1024;
                        }
                    }
                    continue 'next_line;
                }
                State::GpuPowerReadings => {
                    if !l.starts_with("        ") {
                        state = State::InCard;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("        Current Power Limit") {
                        if let Ok(n) = field_value_stripped(l, "W").parse::<f64>() {
                            card.info.power_limit_watt = n.ceil() as i32;
                            card.state.power_limit_watt = card.info.power_limit_watt;
                        }
                    } else if l.starts_with("        Min Power Limit") {
                        if let Ok(n) = field_value_stripped(l, "W").parse::<f64>() {
                            card.info.min_power_limit_watt = n.ceil() as i32;
                        }
                    } else if l.starts_with("        Max Power Limit") {
                        if let Ok(n) = field_value_stripped(l, "W").parse::<f64>() {
                            card.info.max_power_limit_watt = n.ceil() as i32;
                        }
                    } else if l.starts_with("        Power Draw") {
                        if let Ok(n) = field_value_stripped(l, "W").parse::<f64>() {
                            card.state.power_watt = n.ceil() as i32;
                        }
                    }
                    continue 'next_line;
                }
                State::MaxClocks => {
                    if !l.starts_with("        ") {
                        state = State::InCard;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("        SM") {
                        if let Ok(n) = field_value_stripped(l, "MHz").parse::<i32>() {
                            card.info.max_ce_clock_mhz = n;
                        }
                    } else if l.starts_with("        Memory") {
                        if let Ok(n) = field_value_stripped(l, "MHz").parse::<i32>() {
                            card.info.max_mem_clock_mhz = n;
                        }
                    }
                    continue 'next_line;
                }
                State::Clocks => {
                    if !l.starts_with("        ") {
                        state = State::InCard;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("        SM") {
                        if let Ok(n) = field_value_stripped(l, "MHz").parse::<i32>() {
                            card.state.ce_clock_mhz = n;
                        }
                    } else if l.starts_with("        Memory") {
                        if let Ok(n) = field_value_stripped(l, "MHz").parse::<i32>() {
                            card.state.mem_clock_mhz = n;
                        }
                    }
                    continue 'next_line;
                }
                State::Utilization => {
                    if !l.starts_with("        ") {
                        state = State::InCard;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("        Gpu") {
                        if let Ok(n) = field_value_stripped(l, "%").parse::<f32>() {
                            card.state.gpu_utilization_pct = n;
                        }
                    } else if l.starts_with("        Memory") {
                        if let Ok(n) = field_value_stripped(l, "%").parse::<f32>() {
                            card.state.mem_utilization_pct = n;
                        }
                    }
                    continue 'next_line;
                }
                State::Temperature => {
                    if !l.starts_with("        ") {
                        state = State::InCard;
                        continue 'reprocess_line;
                    }
                    if l.starts_with("        GPU Current Temp") {
                        if let Ok(n) = field_value_stripped(l, "C").parse::<i32>() {
                            card.state.temp_c = n;
                        }
                    }
                    continue 'next_line;
                }
            }
        }
    }
    if !card.info.bus_addr.is_empty() {
        cards.push(card);
    }
    cards
}

fn field_value(l: &str) -> String {
    if let Some((_, rest)) = l.split_once(':') {
        rest.trim().to_string()
    } else {
        "".to_string()
    }
}

fn field_value_stripped(l: &str, suffix: &str) -> String {
    if let Some((_, rest)) = l.split_once(':') {
        if let Some(s) = rest.strip_suffix(suffix) {
            return s.trim().to_string();
        }
    }
    "".to_string()
}

// Err(e) really means the command started running but failed, for the reason given.  If the
// command could not be found or no card is present, we return Ok(vec![]).

fn get_nvidia_utilization(user_by_pid: &UserTable) -> Result<Vec<gpu::Process>, String> {
    if !nvidia_present() {
        return Ok(vec![]);
    }
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
//  - it does not show orphaned processes holding onto GPU memory, the way nvtop can do
//
// To fix the former (in part), we parse the line that starts with '# gpu' to get field name
// indices, and then use those indices to fetch data.
//
// To fix the latter problem we do something with --query-compute-apps, see later.
//
// Note that `-c 1 -s mu` gives us more or less instantaneous utilization, not some long-running
// average.
//
// TODO: We could consider using the underlying C library instead, but this adds a fair amount of
// complexity.  See https://docs.nvidia.com/deploy/nvml-api/index.html and the nvidia-smi manual
// page.  This however looks like a bit of a nightmare: it is not installed by default, it must be
// downloaded as part of some cuda kit, the documentation appears to be not great.

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
                return Err("Duplicate header line in pmon output".to_string());
            }
            device_index = Some(0);
            for (i, p) in header_parts.iter().enumerate() {
                match *p {
                    "pid" => {
                        if pid_index.is_some() {
                            return Err("Duplicate pid index".to_string());
                        }
                        pid_index = Some(i - 1)
                    }
                    "fb" => {
                        if mem_size_index.is_some() {
                            return Err("Duplicate fb index".to_string());
                        }
                        mem_size_index = Some(i - 1)
                    }
                    "sm" => {
                        if gpu_util_index.is_some() {
                            return Err("Duplicate sm index".to_string());
                        }
                        gpu_util_index = Some(i - 1)
                    }
                    "mem" => {
                        if mem_util_index.is_some() {
                            return Err("Duplicate mem index".to_string());
                        }
                        mem_util_index = Some(i - 1)
                    }
                    "command" => {
                        if command_index.is_some() {
                            return Err("Duplicate command index".to_string());
                        }
                        command_index = Some(i - 1)
                    }
                    _ => {}
                }
            }
            // Require all indices to come from the same header.
            if device_index.is_none()
                || pid_index.is_none()
                || mem_size_index.is_none()
                || gpu_util_index.is_none()
                || mem_util_index.is_none()
                || command_index.is_none()
            {
                return Err("Missing required field in pmon output".to_string());
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if device_index.is_none() {
            return Err("Missing header in pmon output".to_string());
        }
        let pid_index = pid_index.expect("pid_index already checked");
        let device_index = device_index.expect("device_index already checked");
        let mem_size_index = mem_size_index.expect("mem_size_index already checked");
        let gpu_util_index = gpu_util_index.expect("gpu_util_index already checked");
        let mem_util_index = mem_util_index.expect("mem_util_index already checked");
        let command_index = command_index.expect("command_index already checked");
        let parts = util::chunks(line).1;
        let pid_str = parts[pid_index];
        let device_str = parts[device_index];
        if pid_str == "-" || device_str == "-" {
            // This is definitely not an error but we can't use the data
            continue;
        }
        let pid = match pid_str.parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                // It's possible that this ought to be reported as an error
                continue;
            }
        };
        let device = match device_str.parse::<usize>() {
            Ok(n) => Some(n),
            Err(_) => {
                // It's possible that this ought to be reported as an error
                continue;
            }
        };
        // The following parsers are allowed to fail quietly as missing information may be
        // represented simply as "-", and we're OK with that.
        let mem_size = parts[mem_size_index].parse::<usize>().unwrap_or(0);
        let gpu_util_pct = parts[gpu_util_index].parse::<f64>().unwrap_or(0.0);
        let mem_util_pct = parts[mem_util_index].parse::<f64>().unwrap_or(0.0);
        // For nvidia-smi, we use the first word because the command produces blank-padded
        // output.  We can maybe do better by considering non-empty words.
        let command = parts[command_index].to_string();
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
        return Err("Missing header line in pmon output".to_string());
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

fn parse_query_output(
    raw_text: &str,
    user_by_pid: &UserTable,
) -> Result<Vec<gpu::Process>, String> {
    let mut result = vec![];
    for line in raw_text.lines() {
        let (_start_indices, parts) = util::chunks(line);
        if parts.len() != 2 {
            return Err("Unexpected output from nvidia-smi query: too many fields".to_string());
        }
        let pid_str = parts[0].trim_end_matches(',');
        let pid = match pid_str.parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                return Err(
                    "Unexpected output from nvidia-smi query: first field is not pid".to_string(),
                )
            }
        };
        let mem_usage = match parts[1].parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                return Err(
                    "Unexpected output from nvidia-smi query: second field is not memory size"
                        .to_string(),
                )
            }
        };
        // Do this after parsing to get some sensible syntax checking
        if user_by_pid.contains_key(&pid) {
            continue;
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
    parse_pmon_output(text, &mkusers()).expect("Test: Must have data")
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
    parse_pmon_output(text, &mkusers()).expect("Test: Must have data")
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
        proc! { Some(3), 2233469, "charlie",        1003, 90.0, 23.0, 15771 * 1024, "python3" },
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
        proc! { Some(3), 2233469, "charlie",        1003, 90.0, 23.0, 15771 * 1024, "python3" },
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
    parse_query_output(text, &mkusers()).expect("Test: Must have data")
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

#[test]
fn test_parse_nvidia_configuration() {
    // Some fields in that output have been anonymized and a few have been changed to make it more
    // interesting.
    let cs = parse_nvidia_configuration(std::include_str!("testdata/nvidia-smi-output.txt"));

    // Check # of cards and that they are plausibly independent
    assert!(cs.len() == 4);
    assert!(cs[0].info.bus_addr == "00000000:18:00.0");
    assert!(cs[0].info.index == 0);
    assert!(cs[1].info.bus_addr == "00000000:3B:00.0");
    assert!(cs[1].info.index == 1);
    assert!(cs[2].info.bus_addr == "00000000:86:00.0");
    assert!(cs[2].info.index == 2);
    assert!(cs[3].info.bus_addr == "00000000:AF:00.0");
    assert!(cs[3].info.index == 3);

    // Check details of cs[3] (more interesting than cs[0])
    let c = &cs[3];
    assert!(c.info.model == "NVIDIA GeForce RTX 2080 Ti");
    assert!(c.info.arch == "Turing");
    assert!(c.info.driver == "545.23.08");
    assert!(c.info.firmware == "12.3");
    assert!(c.info.uuid == "GPU-198d6802-0000-0000-0000-000000000000");
    assert!(c.info.mem_size_kib == 11264 * 1024);
    assert!(c.info.power_limit_watt == 250);
    assert!(c.info.max_power_limit_watt == 280);
    assert!(c.info.min_power_limit_watt == 100);
    assert!(c.info.max_ce_clock_mhz == 2100);
    assert!(c.info.max_mem_clock_mhz == 7000);

    assert!(c.state.index == 3);
    assert!(c.state.fan_speed_pct == 28.0);
    assert!(c.state.compute_mode == "Default");
    assert!(c.state.perf_state == "P8");
    assert!(c.state.mem_reserved_kib == 252 * 1024);
    assert!(c.state.mem_used_kib == 3 * 1024);
    assert!(c.state.gpu_utilization_pct == 5.0);
    assert!(c.state.mem_utilization_pct == 8.0);
    assert!(c.state.temp_c == 34);
    assert!(c.state.power_watt == 19); // ceil(18.10)
    assert!(c.state.power_limit_watt == 250);
    assert!(c.state.ce_clock_mhz == 300);
    assert!(c.state.mem_clock_mhz == 405);
}
