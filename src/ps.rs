#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

extern crate log;
extern crate num_cpus;

use crate::amd;
use crate::jobs;
use crate::nvidia;
use crate::process;
use crate::util::{three_places, time_iso8601};

use csv::Writer;
use std::collections::{HashMap, HashSet};
use std::io;

#[cfg(test)]
use crate::util::{map, set};

struct JobInfo {
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_size: usize,
    gpu_cards: HashSet<usize>,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size: usize,
}

fn add_job_info(
    processes_by_job_id: &mut HashMap<(String, usize, String), JobInfo>,
    user: String,
    job_id: usize,
    command: String,
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_size: usize,
    gpu_cards: HashSet<usize>,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size: usize,
) {
    processes_by_job_id
        .entry((user, job_id, command))
        .and_modify(|e| {
            e.cpu_percentage += cpu_percentage;
            e.cputime_sec += cputime_sec;
            e.mem_size += mem_size;
            e.gpu_cards.extend(&gpu_cards);
            e.gpu_percentage += gpu_percentage;
            e.gpu_mem_percentage += gpu_mem_percentage;
            e.gpu_mem_size += gpu_mem_size;
        })
        .or_insert(JobInfo {
            cpu_percentage,
            cputime_sec,
            mem_size,
            gpu_cards,
            gpu_percentage,
            gpu_mem_percentage,
            gpu_mem_size,
        });
}

type Pid = usize;

#[derive(PartialEq)]
struct PsInfo {
    user: String,
    command: String,
    cpu_pct: f64,
    cputime_sec: usize,
    mem_pct: f64,
    mem_size_kib: usize
}

impl PsInfo {
    fn new(user: &str, command: &str, cpu_pct: f64, cputime_sec: usize, mem_pct: f64, mem_size_kib: usize) -> PsInfo {
        PsInfo {
            user: user.to_string(),
            command: command.to_string(),
            cpu_pct,
            cputime_sec,
            mem_pct,
            mem_size_kib
        }
    }
}

fn extract_ps_info(processes: &[process::Process]) -> HashMap<Pid, PsInfo> {
    processes
        .iter()
        .map(
            |process::Process {
                 user,
                 pid,
                 command,
                 cpu_pct,
                 cputime_sec,
                 mem_pct,
                 mem_size_kib,
                 ..
             }| (*pid, PsInfo::new(&user, &command, *cpu_pct, *cputime_sec, *mem_pct, *mem_size_kib)),
        )
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some(ps_info) = acc.get_mut(&key) {
                ps_info.cpu_pct += value.cpu_pct;
                ps_info.cputime_sec += value.cputime_sec;
                ps_info.mem_pct += value.mem_pct;
                ps_info.mem_size_kib += value.mem_size_kib;
            } else {
                acc.insert(key, value);
            }
            acc
        })
}

#[test]
fn test_extract_ps_info() {
    let ps_output = process::parsed_test_output();
    let ps_info = extract_ps_info(&ps_output);

    assert!(
        ps_info
            == map! {
                2022 => PsInfo::new("bob", "slack", 10.0, 60+28, 20.0, 553348),
                42178 => PsInfo::new("bob", "chromium", 20.0, 60+29+60+30, 30.0, 358884),
                42189 => PsInfo::new("alice", "slack", 10.0, 60+31, 5.0, 5528),
                42191 => PsInfo::new("bob", "someapp", 10.0, 60+32, 5.0, 5552),
                42213 => PsInfo::new("alice", "some app", 20.0, 60+33+60+34, 10.0, 484268)
            }
    );
}

#[derive(PartialEq)]
struct GpuInfo {
    user: String,
    command: String,
    gpus: HashSet<usize>,
    gpu_pct: f64,
    gpumem_pct: f64,
    gpumem_size_kib: usize,
}

impl GpuInfo {
    fn new(user: &str, command: &str, gpus: HashSet<usize>, gpu_pct: f64, gpumem_pct: f64, gpumem_size_kib: usize) -> GpuInfo {
        GpuInfo {
            user: user.to_string(),
            command: command.to_string(),
            gpus,
            gpu_pct,
            gpumem_pct,
            gpumem_size_kib
        }
    }
}

fn extract_gpu_info(processes: &[nvidia::Process]) -> HashMap<Pid, GpuInfo> {
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
             }| (*pid, GpuInfo::new(&user, &command, make_gpuset(*device), *gpu_pct, *mem_pct, *mem_size_kib)))
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some(gpu_info) = acc.get_mut(&key) {
                gpu_info.gpus.extend(value.gpus);
                gpu_info.gpu_pct += value.gpu_pct;
                gpu_info.gpumem_pct += value.gpumem_pct;
                gpu_info.gpumem_size_kib += value.gpumem_size_kib;
            } else {
                acc.insert(key, value);
            }
            acc
        })
}

fn make_gpuset(maybe_device: Option<usize>) -> HashSet<usize> {
    let mut gpus = HashSet::new();
    if let Some(dev) = maybe_device {
        gpus.insert(dev);
    }
    gpus
}

fn add_gpu_info(
    processes_by_slurm_job_id: &mut HashMap<(String, usize, String), JobInfo>,
    gpu_output: Result<Vec<nvidia::Process>, String>,
) {
    match gpu_output {
        Ok(gpu_output) => {
            for (pid, gpu_info) in extract_gpu_info(&gpu_output) {
                add_job_info(
                    processes_by_slurm_job_id,
                    gpu_info.user,
                    pid,
                    gpu_info.command,
                    0.0,
                    0,
                    0,
                    gpu_info.gpus,
                    gpu_info.gpu_pct,
                    gpu_info.gpumem_pct,
                    gpu_info.gpumem_size_kib,
                );
            }
        }
        Err(e) => {
            log::error!("GPU process listing failed: {}", e);
        }
    }
}

#[test]
fn test_extract_nvidia_pmon_processes() {
    let ps_output = nvidia::parsed_pmon_output();
    let gpu_info = extract_gpu_info(&ps_output);

    assert!(
        gpu_info
            == map! {
                447153 => GpuInfo::new("bob", "python3.9", set!{0}, 0.0, 0.0, 7669*1024),
                447160 => GpuInfo::new("bob", "python3.9", set!{0}, 0.0, 0.0, 11057*1024),
                506826 => GpuInfo::new("_zombie_506826", "python3.9", set!{0}, 0.0, 0.0, 11057*1024),
                1864615 => GpuInfo::new("alice", "python", set!{0, 1, 2, 3}, 40.0, 0.0, (1635+535+535+535)*1024),
                2233095 => GpuInfo::new("charlie", "python3", set!{1}, 84.0, 23.0, 24395*1024),
                1448150 => GpuInfo::new("_zombie_1448150", "python3", set!{2}, 0.0, 0.0, 9383*1024),
                2233469 => GpuInfo::new("charlie", "python3", set!{3}, 90.0, 23.0, 15771*1024)
            }
    );
}

#[test]
fn test_extract_nvidia_query_processes() {
    let ps_output = nvidia::parsed_query_output();
    let gpu_info = extract_gpu_info(&ps_output);

    assert!(
        gpu_info
            == map! {
                3079002 => GpuInfo::new("_zombie_3079002", "_unknown_", HashSet::new(), 0.0, 0.0, 2350*1024)
            }
    );
}

pub fn create_snapshot(
    jobs: &mut dyn jobs::JobManager,
    cpu_cutoff_percent: f64,
    mem_cutoff_percent: f64,
) {
    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();

    let mut processes_by_job_id: HashMap<(String, usize, String), JobInfo> = HashMap::new();
    let mut user_by_pid: HashMap<usize, String> = HashMap::new();

    match process::get_process_information(jobs) {
        Err(e) => {
            log::error!("CPU process listing failed: {:?}", e);
            return;
        }
        Ok(ps_output) => {
            for (pid, ps_info) in extract_ps_info(&ps_output)
            {
                user_by_pid.insert(pid, ps_info.user.clone());

                if (ps_info.cpu_pct >= cpu_cutoff_percent) || (ps_info.mem_pct >= mem_cutoff_percent)
                {
                    add_job_info(
                        &mut processes_by_job_id,
                        ps_info.user,
                        jobs.job_id_from_pid(pid, &ps_output),
                        ps_info.command,
                        ps_info.cpu_pct,
                        ps_info.cputime_sec,
                        ps_info.mem_size_kib,
                        HashSet::new(),
                        0.0,
                        0.0,
                        0,
                    );
                }
            }
        }
    }

    add_gpu_info(
        &mut processes_by_job_id,
        nvidia::get_nvidia_information(&user_by_pid),
    );
    add_gpu_info(
        &mut processes_by_job_id,
        amd::get_amd_information(&user_by_pid),
    );

    let mut writer = Writer::from_writer(io::stdout());

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    for ((user, job_id, command), job_info) in processes_by_job_id {
        // "unknown" is not implemented, see https://github.com/NordicHPC/sonar/issues/75
        let mut gpus_comma_separated: String = job_info
            .gpu_cards
            .iter()
            .map(|&num| num.to_string())
            .collect::<Vec<String>>()
            .join(",");

        if gpus_comma_separated.is_empty() {
            gpus_comma_separated = "none".to_string();
        }

        writer
            .write_record([
                &format!("v={VERSION}"),
                &format!("time={timestamp}"),
                &format!("host={hostname}"),
                &format!("cores={num_cores}"),
                &format!("user={user}"),
                &format!("job={job_id}"),
                &format!("cmd={command}"),
                &format!("cpu%={}", three_places(job_info.cpu_percentage)),
                &format!("cpukib={}", job_info.mem_size),
                &format!("gpus={}", gpus_comma_separated),
                &format!("gpu%={}", three_places(job_info.gpu_percentage)),
                &format!("gpumem%={}", three_places(job_info.gpu_mem_percentage)),
                &format!("gpukib={}", job_info.gpu_mem_size),
                &format!("cputime_sec={}", job_info.cputime_sec),
            ])
            .unwrap();
    }

    writer.flush().unwrap();
}
