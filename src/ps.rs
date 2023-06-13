#![allow(clippy::type_complexity)]

use crate::command;
use chrono::prelude::{DateTime, Utc};
use std::collections::HashMap;
extern crate num_cpus;
use csv::Writer;
use std::io;

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

const PS_COMMAND : &str = "ps -e --no-header -o pid,user:22,pcpu,pmem,size,comm | grep -v ' 0.0  0.0 '";

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

#[cfg(test)]
mod test {
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

const NVIDIA_SMI_COMMAND : &str = "nvidia-smi pmon -c 1";

// The key for the ht is (user, pid, command) and it would be nice if
// we could get that, though we don't have a user name here.  It would
// need to be looked up.
//
// The value is likely gpu utilization, memory utilization, and memory
// size.  For that we'd want total memory, which means basically
// looking it up separately using --query.
//
// What do we do for a system with 8 cards, say?
//
// The percentage shown by nvidia-smi is per card.  So I guess if an
// application used all eight cards, it can use up to 800% GPU and
// 800% memory, and the memory size is just the sum of the memories.

/* There are various commands that could work, but this one is a good start:

[larstha@ml3 ~]$ nvidia-smi pmon -c 1
# gpu        pid  type    sm   mem   enc   dec   command
# Idx          #   C/G     %     %     %     %   name
    0    3263942     C     -     -     -     -   python         
    0    3334452     C     1     0     -     -   python         
    1          -     -     -     -     -     -   -              
    2          -     -     -     -     -     -   -              
    3    2642685     C    70    65     -     -   python         
    3    3322026     C     -     -     -     -   python         
*/

// Probably want gpu utilization, memory utilization?  For this
// we may have to get the user from the pid, if we can.

fn extract_nvidia_processes(raw_text: &str) -> HashMap<(String, String, String), (f64, f64, usize)> {
    // This should deal with nvidia-smi not being present and should
    // just return an empty map if so.  We could have a similar
    // function for rocm-smi.
    let result = raw_text
        .lines()
	.filter(|line| !line.starts_with("#"))
        .map(|line| chunks(line))
	.filter(|(_, parts)| parts[1] != "-")
        .map(|(start_indices, parts)| {
            let device = parts[0].parse::<usize>().unwrap();
            let pid = parts[1].parse::<usize>().unwrap();
            let maybe_gpu_pct = parts[3].parse::<f64>();
            let maybe_mem_pct = parts[4].parse::<f64>();

            // this is done because command can have spaces
            let command = parts[7];  // FIXME

	    // FIXME: Map PID to user
	    let user = "somebody";
            (
                (user.to_string(), pid.to_string(), command.to_string()),
                (maybe_gpu_pct.unwrap_or(0.0),
		 maybe_mem_pct.unwrap_or(0.0),
		 0usize /* FIXME */),
            )
        })
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((gpu_pct, mem_pct, mem_size)) = acc.get_mut(&key) {
                *gpu_pct += value.0;
                *mem_pct += value.1;
                *mem_size += value.2;
            } else {
                acc.insert(key, value);
            }
            acc
        });
    result
}

struct JobInfo {
    cpu_percentage: f64,
    mem_size: usize,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size: usize,
}

// round to 3 decimal places
fn three_places(n : f64) -> f64 {
    (n * 1000.0).round() / 1000.0
}

pub fn create_snapshot(cpu_cutoff_percent: f64, mem_cutoff_percent: f64) {
    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();

    // the pipe is here as a workaround for https://github.com/rust-lang/rust/issues/45572
    // see also https://doc.rust-lang.org/std/process/index.html
    let timeout_seconds = 2;

    let mut processes_by_slurm_job_id: HashMap<(String, usize, String), JobInfo> =
        HashMap::new();

    if let Some(out) = command::safe_command(PS_COMMAND, timeout_seconds) {
        for ((user, pid, command), (cpu_percentage, mem_percentage, mem_size)) in
	    extract_ps_processes(&out)
	{
            if (cpu_percentage >= cpu_cutoff_percent) || (mem_percentage >= mem_cutoff_percent) {
                let slurm_job_id = get_slurm_job_id(pid).unwrap_or_default();
                let slurm_job_id_usize = slurm_job_id.trim().parse::<usize>().unwrap_or_default();

                processes_by_slurm_job_id
                    .entry((user, slurm_job_id_usize, command))
                    .and_modify(|e| {
                        e.cpu_percentage += cpu_percentage;
                        e.mem_size += mem_size;
                    })
                    .or_insert(JobInfo { cpu_percentage,
					 mem_size,
					 gpu_percentage: 0.0,
					 gpu_mem_percentage: 0.0,
					 gpu_mem_size: 0 });
            }
        }
    }

    if let Some(out) = command::safe_command(NVIDIA_SMI_COMMAND, timeout_seconds) {
	for ((user, pid, command), (gpu_percentage, gpu_mem_percentage, gpu_mem_size)) in
	    extract_nvidia_processes(&out)
	{
	    // I think generally we want to not filter processes here?
            let slurm_job_id = get_slurm_job_id(pid).unwrap_or_default();
            let slurm_job_id_usize = slurm_job_id.trim().parse::<usize>().unwrap_or_default();

            processes_by_slurm_job_id
                .entry((user, slurm_job_id_usize, command))
                .and_modify(|e| {
                    e.gpu_percentage += gpu_percentage;
                    e.gpu_mem_percentage += gpu_mem_percentage;
                    e.gpu_mem_size += gpu_mem_size;
                })
                .or_insert(JobInfo { cpu_percentage: 0.0,
				     mem_size: 0,
				     gpu_percentage,
				     gpu_mem_percentage,
				     gpu_mem_size });
	}
    }

    let mut writer = Writer::from_writer(io::stdout());

    for ((user, slurm_job_id, command), job_info) in processes_by_slurm_job_id
    {
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
		&three_places(job_info.gpu_percentage).to_string(),
		&three_places(job_info.gpu_mem_percentage).to_string(),
		&job_info.gpu_mem_size.to_string()
            ])
            .unwrap();
    }

    writer.flush().unwrap();
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
