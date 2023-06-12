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

fn extract_processes(raw_text: &str) -> HashMap<(String, String, String), (f64, f64, usize)> {
    let result = raw_text
        .lines()
        .map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next().unwrap();
            let user = parts.next().unwrap();
            let cpu = parts.next().unwrap().parse::<f64>().unwrap();
            let mem = parts.next().unwrap().parse::<f64>().unwrap();
            let size = parts.next().unwrap().parse::<usize>().unwrap();
            let command = parts.next().unwrap();

            (
                (user.to_string(), pid.to_string(), command.to_string()),
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
    fn test_extract_processes() {
        let text = "   2022 bob                            10.0 20.0 553348 slack
  42178 bob                            10.0 15.0 353348 chromium
  42178 bob                            10.0 15.0  5536 chromium
  42189 alice                          10.0  5.0  5528 slack
  42191 bob                            10.0  5.0  5552 someapp
  42213 alice                          10.0  5.0 348904 someapp
  42213 alice                          10.0  5.0 135364 someapp";

        let processes = extract_processes(text);

        assert!(
            processes
                == map! {
                    ("bob".to_string(), "2022".to_string(), "slack".to_string()) => (10.0, 20.0, 553348),
                    ("bob".to_string(), "42178".to_string(), "chromium".to_string()) => (20.0, 30.0, 358884),
                    ("alice".to_string(), "42189".to_string(), "slack".to_string()) => (10.0, 5.0, 5528),
                    ("bob".to_string(), "42191".to_string(), "someapp".to_string()) => (10.0, 5.0, 5552),
                    ("alice".to_string(), "42213".to_string(), "someapp".to_string()) => (20.0, 10.0, 484268)
                }
        );
    }
}

pub fn create_snapshot(cpu_cutoff_percent: f64, mem_cutoff_percent: f64) {
    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();

    // the pipe is here as a workaround for https://github.com/rust-lang/rust/issues/45572
    // see also https://doc.rust-lang.org/std/process/index.html
    let command = "ps -e --no-header -o pid,user:22,pcpu,pmem,size,comm | grep -v ' 0.0  0.0 '";
    let timeout_seconds = 2;

    let output = command::safe_command(command, timeout_seconds);

    let mut processes_by_slurm_job_id: HashMap<(String, usize, String), (f64, usize)> =
        HashMap::new();

    if let Some(out) = output {
        let processes = extract_processes(&out);

        for ((user, pid, command), (cpu_percentage, mem_percentage, mem_size)) in processes {
            if (cpu_percentage >= cpu_cutoff_percent) || (mem_percentage >= mem_cutoff_percent) {
                let slurm_job_id = get_slurm_job_id(pid).unwrap_or_default();
                let slurm_job_id_usize = slurm_job_id.trim().parse::<usize>().unwrap_or_default();

                processes_by_slurm_job_id
                    .entry((user, slurm_job_id_usize, command))
                    .and_modify(|e| {
                        e.0 += cpu_percentage;
                        e.1 += mem_size;
                    })
                    .or_insert((cpu_percentage, mem_size));
            }
        }

        let mut writer = Writer::from_writer(io::stdout());

        for ((user, slurm_job_id, command), (cpu_percentage, mem_size)) in processes_by_slurm_job_id
        {
            // round cpu_percentage to 3 decimal places
            let cpu_percentage = (cpu_percentage * 1000.0).round() / 1000.0;

            writer
                .write_record([
                    &timestamp,
                    &hostname,
                    &num_cores.to_string(),
                    &user,
                    &slurm_job_id.to_string(),
                    &command,
                    &cpu_percentage.to_string(),
                    &mem_size.to_string(),
                ])
                .unwrap();
        }

        writer.flush().unwrap();
    };
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
