#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::amd;
use crate::command;
use crate::nvidia;
use crate::process;
use crate::util::{three_places, time_iso8601};
use std::collections::HashMap;
extern crate num_cpus;
#[cfg(test)]
use crate::util::map;
use csv::Writer;
use std::io;

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

fn extract_ps_processes(
    processes: &[process::Process],
) -> HashMap<(String, String, String), (f64, f64, usize)> {
    processes
        .iter()
        .map(
            |process::Process {
                 user,
                 pid,
                 command,
                 cpu_pct,
                 mem_pct,
                 mem_size_kib,
                 ..
             }| {
                (
                    (user.clone(), pid.clone(), command.clone()),
                    (*cpu_pct, *mem_pct, *mem_size_kib),
                )
            },
        )
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((cpu_pct, mem_pct, mem_size_kib)) = acc.get_mut(&key) {
                *cpu_pct += value.0;
                *mem_pct += value.1;
                *mem_size_kib += value.2;
            } else {
                acc.insert(key, value);
            }
            acc
        })
}

#[test]
fn test_extract_ps_processes() {
    let ps_output = process::parsed_test_output();
    let processes = extract_ps_processes(&ps_output);

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

fn extract_nvidia_processes(
    processes: &[nvidia::Process],
) -> HashMap<(String, String, String), (u32, f64, f64, usize)> {
    processes
        .iter()
        .map(
            |nvidia::Process {
                 device,
                 pid,
                 user,
                 gpu_pct,
                 mem_pct,
                 mem_size_kib,
                 command,
             }| {
                (
                    (user.clone(), pid.clone(), command.clone()),
                    (
                        if *device >= 0 { 1 << device } else { !0 },
                        *gpu_pct,
                        *mem_pct,
                        *mem_size_kib,
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
        })
}

fn add_gpu_info(
    processes_by_slurm_job_id: &mut HashMap<(String, usize, String), JobInfo>,
    gpu_output: Vec<nvidia::Process>,
) {
    for ((user, pid, command), (gpu_mask, gpu_percentage, gpu_mem_percentage, gpu_mem_size)) in
        extract_nvidia_processes(&gpu_output)
    {
        add_job_info(
            processes_by_slurm_job_id,
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
}

#[test]
fn test_extract_nvidia_pmon_processes() {
    let ps_output = nvidia::parsed_pmon_output();
    let processes = extract_nvidia_processes(&ps_output);

    assert!(
        processes
            == map! {
                ("bob".to_string(), "447153".to_string(), "python3.9".to_string())            => (0b1, 0.0, 0.0, 7669*1024),
                ("bob".to_string(), "447160".to_string(), "python3.9".to_string())            => (0b1, 0.0, 0.0, 11057*1024),
                ("_zombie_506826".to_string(), "506826".to_string(), "python3.9".to_string()) => (0b1, 0.0, 0.0, 11057*1024),
                ("alice".to_string(), "1864615".to_string(), "python".to_string())            => (0b1111, 40.0, 0.0, (1635+535+535+535)*1024),
                ("charlie".to_string(), "2233095".to_string(), "python3".to_string())         => (0b10, 84.0, 23.0, 24395*1024),
                ("_zombie_1448150".to_string(), "1448150".to_string(), "python3".to_string()) => (0b100, 0.0, 0.0, 9383*1024),
                ("charlie".to_string(), "2233469".to_string(), "python3".to_string())         => (0b1000, 90.0, 23.0, 15771*1024)
            }
    );
}

#[test]
fn test_extract_nvidia_query_processes() {
    let ps_output = nvidia::parsed_query_output();
    let processes = extract_nvidia_processes(&ps_output);

    assert!(
        processes
            == map! {
                ("_zombie_3079002".to_string(), "3079002".to_string(), "_unknown_".to_string()) => (!0, 0.0, 0.0, 2350*1024)
            }
    );
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

    let ps_output = process::get_process_information(timeout_seconds);
    for ((user, pid, command), (cpu_percentage, mem_percentage, mem_size)) in
        extract_ps_processes(&ps_output)
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

    add_gpu_info(
        &mut processes_by_slurm_job_id,
        nvidia::get_nvidia_information(timeout_seconds, &user_by_pid),
    );
    add_gpu_info(
        &mut processes_by_slurm_job_id,
        amd::get_amd_information(&user_by_pid),
    );

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
