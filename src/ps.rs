#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::command;
use crate::util;
use chrono::prelude::{DateTime, Utc};
use std::collections::HashMap;
extern crate num_cpus;
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
