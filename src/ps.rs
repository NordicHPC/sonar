#![allow(clippy::type_complexity)]

use crate::command;
use chrono::prelude::{DateTime, Utc};
use std::collections::HashMap;
extern crate num_cpus;

fn time_iso8601() -> String {
    let dt: DateTime<Utc> = std::time::SystemTime::now().into();
    format!("{}", dt.format("%+"))
}

fn extract_processes(raw_text: &str) -> HashMap<(String, String), (f64, f64, usize)> {
    let result = raw_text
        .lines()
        .map(|line| {
            let mut parts = line.split_whitespace();
            let _pid = parts.next().unwrap();
            let user = parts.next().unwrap();
            let cpu = parts.next().unwrap().parse::<f64>().unwrap();
            let mem = parts.next().unwrap().parse::<f64>().unwrap();
            let size = parts.next().unwrap().parse::<usize>().unwrap();
            let command = parts.next().unwrap();

            ((user.to_string(), command.to_string()), (cpu, mem, size))
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
  42188 bob                            10.0 15.0  5536 chromium
  42189 alice                          10.0  5.0  5528 slack
  42191 bob                            10.0  5.0  5552 someapp
  42213 alice                          10.0  5.0 348904 someapp
  42214 alice                          10.0  5.0 135364 someapp";

        let processes = extract_processes(text);

        assert!(
            processes
                == map! {
                    ("bob".to_string(), "slack".to_string()) => (10.0, 20.0, 553348),
                    ("bob".to_string(), "chromium".to_string()) => (20.0, 30.0, 358884),
                    ("alice".to_string(), "slack".to_string()) => (10.0, 5.0, 5528),
                    ("bob".to_string(), "someapp".to_string()) => (10.0, 5.0, 5552),
                    ("alice".to_string(), "someapp".to_string()) => (20.0, 10.0, 484268)
                }
        );
    }
}

pub fn create_snapshot(
    cpu_cutoff_percent: f64,
    mem_cutoff_percent: f64,
    mem_cutoff_percent_idle: f64,
) {
    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();

    let command = "ps";
    let args = vec!["-e", "--no-header", "-o", "pid,user:22,pcpu,pmem,size,comm"];
    let timeout_seconds = 2;

    let output = command::safe_command(command, args, timeout_seconds);

    if let Some(out) = output {
        let processes = extract_processes(&out);

        for ((user, command), (cpu_percentage, mem_percentage, mem_size)) in processes {
            if (cpu_percentage >= cpu_cutoff_percent && mem_percentage >= mem_cutoff_percent)
                || mem_percentage >= mem_cutoff_percent_idle
            {
                // round cpu_percentage to 3 decimal places
                let cpu_percentage = (cpu_percentage * 1000.0).round() / 1000.0;

                println!(
                  "{timestamp},{hostname},{num_cores},{user},{command},{cpu_percentage},{mem_size}"
              );
            }
        }
    };
}
