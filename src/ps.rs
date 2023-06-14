#![allow(clippy::type_complexity)]

use crate::command;
use chrono::prelude::{DateTime, Utc};
use std::collections::HashMap;
extern crate num_cpus;
use csv::Writer;
use std::io;

// Populate a HashMap
#[cfg(test)]
macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

fn time_iso8601() -> String {
    let dt: DateTime<Utc> = std::time::SystemTime::now().into();
    format!("{}", dt.format("%+"))
}

fn chunks(input: &str) -> (Vec<usize>, Vec<&str>) {
    let mut start_indices: Vec<usize> = Vec::new();
    let mut parts: Vec<&str> = Vec::new();

    let mut last_index = 0;
    for (index, c) in input.char_indices() {
        if c.is_whitespace() {
            if last_index != index {
                start_indices.push(last_index);
                parts.push(&input[last_index..index]);
            }
            last_index = index + 1;
        }
    }

    if last_index < input.len() {
        start_indices.push(last_index);
        parts.push(&input[last_index..]);
    }

    (start_indices, parts)
}

const PS_COMMAND: &str =
    "ps -e --no-header -o pid,user:22,pcpu,pmem,size,comm | grep -v ' 0.0  0.0 '";

fn extract_ps_processes(raw_text: &str) -> HashMap<(String, String, String), (f64, f64, usize)> {
    let result = raw_text
        .lines()
        .map(|line| {
            let (start_indices, parts) = chunks(line);

            let pid = parts[0];
            let user = parts[1];
            let cpu = parts[2].parse::<f64>().unwrap();
            let mem = parts[3].parse::<f64>().unwrap();
            let size = parts[4].parse::<usize>().unwrap();

            // this is done because command can have spaces
            let command = line[start_indices[5]..].to_string();

            (
                (user.to_string(), pid.to_string(), command),
                (cpu, mem, size),
            )
        })
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((cpu, mem, size)) = acc.get_mut(&key) {
                *cpu += value.0;
                *mem += value.1;
                *size += value.2;
            } else {
                acc.insert(key, value);
            }
            acc
        });

    result
}

#[cfg(test)]
mod test_ps {
    use super::*;

    #[test]
    fn test_extract_ps_processes() {
        let text = "   2022 bob                            10.0 20.0 553348 slack
  42178 bob                            10.0 15.0 353348 chromium
  42178 bob                            10.0 15.0  5536 chromium
  42189 alice                          10.0  5.0  5528 slack
  42191 bob                            10.0  5.0  5552 someapp
  42213 alice                          10.0  5.0 348904 some app
  42213 alice                          10.0  5.0 135364 some app";

        let processes = extract_ps_processes(text);

        assert!(
            processes
                == map! {
                    ("bob".to_string(), "2022".to_string(), "slack".to_string()) => (10.0, 20.0, 553348),
                    ("bob".to_string(), "42178".to_string(), "chromium".to_string()) => (20.0, 30.0, 358884),
                    ("alice".to_string(), "42189".to_string(), "slack".to_string()) => (10.0, 5.0, 5528),
                    ("bob".to_string(), "42191".to_string(), "someapp".to_string()) => (10.0, 5.0, 5552),
                    ("alice".to_string(), "42213".to_string(), "some app".to_string()) => (20.0, 10.0, 484268)
                }
        );
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
//
// TODO: Maybe #ifdef all this NVIDIA stuff on a build config that is NVIDIA-specific?

const NVIDIA_PMON_COMMAND: &str = "nvidia-smi pmon -c 1 -s mu";

// Returns (user-name, pid, command-name) -> (device-mask, gpu-util-pct, gpu-mem-pct, gpu-mem-size-in-kib)
// where the values are summed across all devices and the device-mask is a bitmask for the
// GPU devices used by that process.  For a system with 8 cards, utilization
// can reach 800% and the memory size can reach the sum of the memories on the cards.

fn extract_nvidia_pmon_processes(
    raw_text: &str,
    user_by_pid: &HashMap<String, String>,
) -> HashMap<(String, String, String), (u32, f64, f64, usize)> {
    let result = raw_text
        .lines()
        .filter(|line| !line.starts_with("#"))
        .map(|line| {
            let (_start_indices, parts) = chunks(line);
            let device = parts[0].parse::<usize>().unwrap();
            let pid = parts[1];
            let maybe_mem_usage = parts[3].parse::<usize>();
            let maybe_gpu_pct = parts[4].parse::<f64>();
            let maybe_mem_pct = parts[5].parse::<f64>();
            // For nvidia-smi, we use the first word because the command produces
            // blank-padded output.  We can maybe do better by considering non-empty words.
            let command = parts[8].to_string();
            let user = match user_by_pid.get(pid) {
                Some(name) => name.clone(),
                None => "_zombie_".to_owned() + pid,
            };
            (
                pid,
                device,
                user,
                maybe_mem_usage,
                maybe_gpu_pct,
                maybe_mem_pct,
                command,
            )
        })
        .filter(|(pid, ..)| *pid != "-")
        .map(
            |(pid, device, user, maybe_mem_usage, maybe_gpu_pct, maybe_mem_pct, command)| {
                (
                    (user.to_string(), pid.to_string(), command.to_string()),
                    (
                        1 << device,
                        maybe_gpu_pct.unwrap_or(0.0),
                        maybe_mem_pct.unwrap_or(0.0),
                        maybe_mem_usage.unwrap_or(0usize) * 1024,
                    ),
                )
            },
        )
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((device, gpu_pct, mem_pct, mem_size)) = acc.get_mut(&key) {
                *device |= value.0;
                *gpu_pct += value.1;
                *mem_pct += value.2;
                *mem_size += value.3;
            } else {
                acc.insert(key, value);
            }
            acc
        });
    result
}

// We use this to get information about processes that are not captured by pmon.  It's hacky
// but it works.

const NVIDIA_QUERY_COMMAND: &str =
    "nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits";

// Same signature as extract_nvidia_pmon_processes(), q.v. but user is always "_zombie_" and command
// is always "_unknown_".  Only pids not in user_by_pid are returned.

fn extract_nvidia_query_processes(
    raw_text: &str,
    user_by_pid: &HashMap<String, String>,
) -> HashMap<(String, String, String), (u32, f64, f64, usize)> {
    let result = raw_text
        .lines()
        .map(|line| {
            let (_start_indices, parts) = chunks(line);
            let pid = parts[0].strip_suffix(",").unwrap();
            let mem_usage = parts[1].parse::<usize>().unwrap();
            let user = "_zombie_".to_owned() + pid;
            let command = "_unknown_";
            (
                (user.to_string(), pid.to_string(), command.to_string()),
                (!0, 0.0, 0.0, mem_usage * 1024),
            )
        })
        .filter(|((_, pid, _), _)| !user_by_pid.contains_key(pid))
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((device, _gpu_pct, _mem_pct, mem_size)) = acc.get_mut(&key) {
                *device |= value.0;
                *mem_size += value.3;
            } else {
                acc.insert(key, value);
            }
            acc
        });
    result
}

// Shared test cases for the NVIDIA stuff

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

struct JobInfo {
    cpu_percentage: f64,
    mem_size: usize,
    gpu_mask: u32, // Up to 32 GPUs, good enough for now?
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size: usize,
}

fn add_job_info(
    processes_by_slurm_job_id: &mut HashMap<(String, usize, String), JobInfo>,
    user: String,
    pid: String,
    command: String,
    cpu_percentage: f64,
    mem_size: usize,
    gpu_mask: u32,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size: usize,
) {
    let slurm_job_id = get_slurm_job_id(pid).unwrap_or_default();
    let slurm_job_id_usize = slurm_job_id.trim().parse::<usize>().unwrap_or_default();

    processes_by_slurm_job_id
        .entry((user, slurm_job_id_usize, command))
        .and_modify(|e| {
            e.cpu_percentage += cpu_percentage;
            e.mem_size += mem_size;
            e.gpu_mask |= gpu_mask;
            e.gpu_percentage += gpu_percentage;
            e.gpu_mem_percentage += gpu_mem_percentage;
            e.gpu_mem_size += gpu_mem_size;
        })
        .or_insert(JobInfo {
            cpu_percentage,
            mem_size,
            gpu_mask,
            gpu_percentage,
            gpu_mem_percentage,
            gpu_mem_size,
        });
}

pub fn create_snapshot(cpu_cutoff_percent: f64, mem_cutoff_percent: f64) {
    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();

    // the pipe is here as a workaround for https://github.com/rust-lang/rust/issues/45572
    // see also https://doc.rust-lang.org/std/process/index.html
    let timeout_seconds = 2;

    let mut processes_by_slurm_job_id: HashMap<(String, usize, String), JobInfo> = HashMap::new();

    let mut user_by_pid: HashMap<String, String> = HashMap::new();

    if let Some(out) = command::safe_command(PS_COMMAND, timeout_seconds) {
        for ((user, pid, command), (cpu_percentage, mem_percentage, mem_size)) in
            extract_ps_processes(&out)
        {
            user_by_pid.insert(pid.clone(), user.clone());

            if (cpu_percentage >= cpu_cutoff_percent) || (mem_percentage >= mem_cutoff_percent) {
                add_job_info(
                    &mut processes_by_slurm_job_id,
                    user,
                    pid,
                    command,
                    cpu_percentage,
                    mem_size,
                    0,
                    0.0,
                    0.0,
                    0,
                );
            }
        }
    }

    if let Some(out) = command::safe_command(NVIDIA_PMON_COMMAND, timeout_seconds) {
        for ((user, pid, command), (gpu_mask, gpu_percentage, gpu_mem_percentage, gpu_mem_size)) in
            extract_nvidia_pmon_processes(&out, &user_by_pid)
        {
            add_job_info(
                &mut processes_by_slurm_job_id,
                user,
                pid,
                command,
                0.0,
                0,
                gpu_mask,
                gpu_percentage,
                gpu_mem_percentage,
                gpu_mem_size,
            );
        }

        // nvidia-smi worked, so look for orphans processes not caught by pmon
        // For these, "user" will be "_zombie_PID" and command will be "_unknown_", and
        // even though GPU memory percentage is 0.0 there's a nonzero number for
        // the memory usage.  All we care about is really the visibility.

        if let Some(out) = command::safe_command(NVIDIA_QUERY_COMMAND, timeout_seconds) {
            for ((user, pid, command), (gpu_mask, _, _, gpu_mem_size)) in
                extract_nvidia_query_processes(&out, &user_by_pid)
            {
                add_job_info(
                    &mut processes_by_slurm_job_id,
                    user,
                    pid,
                    command,
                    0.0,
                    0,
                    gpu_mask,
                    0.0,
                    0.0,
                    gpu_mem_size,
                );
            }
        }
    }

    let mut writer = Writer::from_writer(io::stdout());

    for ((user, slurm_job_id, command), job_info) in processes_by_slurm_job_id {
        writer
            .write_record([
                &timestamp,
                &hostname,
                &num_cores.to_string(),
                &user,
                &slurm_job_id.to_string(),
                &command,
                &three_places(job_info.cpu_percentage).to_string(),
                &job_info.mem_size.to_string(),
                // TODO: There are other sensible formats for the device mask, notably
                // non-numeric strings, strings padded on the left out to the number
                // of devices on the system, and strings of device numbers separated by
                // some non-comma separator char eg "7:2:1".
                &format!("{:b}", job_info.gpu_mask),
                &three_places(job_info.gpu_percentage).to_string(),
                &three_places(job_info.gpu_mem_percentage).to_string(),
                &job_info.gpu_mem_size.to_string(),
            ])
            .unwrap();
    }

    writer.flush().unwrap();
}

// round to 3 decimal places
fn three_places(n: f64) -> f64 {
    (n * 1000.0).round() / 1000.0
}

fn get_slurm_job_id(pid: String) -> Option<String> {
    let path = format!("/proc/{}/cgroup", pid);

    if !std::path::Path::new(&path).exists() {
        return None;
    }

    let command = format!(
        "cat /proc/{}/cgroup | grep -oP '(?<=job_).*?(?=/)' | head -n 1",
        pid
    );
    let timeout_seconds = 2;

    command::safe_command(&command, timeout_seconds)
}
