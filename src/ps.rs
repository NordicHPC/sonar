#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::amd;
use crate::jobs;
use crate::nvidia;
use crate::process;
use crate::util::{three_places, time_iso8601};
use std::collections::{HashMap, HashSet};

extern crate log;
extern crate num_cpus;

use csv::Writer;
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

fn extract_ps_processes(
    processes: &[process::Process],
) -> HashMap<(String, usize, String), (f64, usize, f64, usize)> {
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
             }| {
                (
                    (user.clone(), *pid, command.clone()),
                    (*cpu_pct, *cputime_sec, *mem_pct, *mem_size_kib),
                )
            },
        )
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((cpu_pct, cputime_sec, mem_pct, mem_size_kib)) = acc.get_mut(&key) {
                *cpu_pct += value.0;
                *cputime_sec += value.1;
                *mem_pct += value.2;
                *mem_size_kib += value.3;
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
                ("bob".to_string(), 2022, "slack".to_string()) => (10.0, 60+28, 20.0, 553348),
                ("bob".to_string(), 42178, "chromium".to_string()) => (20.0, 60+29+60+30, 30.0, 358884),
                ("alice".to_string(), 42189, "slack".to_string()) => (10.0, 60+31, 5.0, 5528),
                ("bob".to_string(), 42191, "someapp".to_string()) => (10.0, 60+32, 5.0, 5552),
                ("alice".to_string(), 42213, "some app".to_string()) => (20.0, 60+33+60+34, 10.0, 484268)
            }
    );
}

fn extract_nvidia_processes(
    processes: &[nvidia::Process],
) -> HashMap<(String, usize, String), (HashSet<usize>, f64, f64, usize)> {
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
                    (user.clone(), *pid, command.clone()),
                    (*device, *gpu_pct, *mem_pct, *mem_size_kib),
                )
            },
        )
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((gpu_cards, gpu_pct, mem_pct, mem_size)) = acc.get_mut(&key) {
                if value.0.is_some() {
                    gpu_cards.insert(value.0.unwrap());
                }
                *gpu_pct += value.1;
                *mem_pct += value.2;
                *mem_size += value.3;
            } else {
                let mut gpu_cards = HashSet::new();
                if value.0.is_some() {
                    gpu_cards.insert(value.0.unwrap());
                }
                acc.insert(key, (gpu_cards, value.1, value.2, value.3));
            }
            acc
        })
}

fn add_gpu_info(
    processes_by_slurm_job_id: &mut HashMap<(String, usize, String), JobInfo>,
    gpu_output: Result<Vec<nvidia::Process>, String>,
) {
    match gpu_output {
        Ok(gpu_output) => {
            for (
                (user, pid, command),
                (gpu_cards, gpu_percentage, gpu_mem_percentage, gpu_mem_size),
            ) in extract_nvidia_processes(&gpu_output)
            {
                add_job_info(
                    processes_by_slurm_job_id,
                    user,
                    pid,
                    command,
                    0.0,
                    0,
                    0,
                    gpu_cards,
                    gpu_percentage,
                    gpu_mem_percentage,
                    gpu_mem_size,
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
    let processes = extract_nvidia_processes(&ps_output);

    assert!(
        processes
            == map! {
                ("bob".to_string(), 447153, "python3.9".to_string())            => (set!{0}, 0.0, 0.0, 7669*1024),
                ("bob".to_string(), 447160, "python3.9".to_string())            => (set!{0}, 0.0, 0.0, 11057*1024),
                ("_zombie_506826".to_string(), 506826, "python3.9".to_string()) => (set!{0}, 0.0, 0.0, 11057*1024),
                ("alice".to_string(), 1864615, "python".to_string())            => (set!{0, 1, 2, 3}, 40.0, 0.0, (1635+535+535+535)*1024),
                ("charlie".to_string(), 2233095, "python3".to_string())         => (set!{1}, 84.0, 23.0, 24395*1024),
                ("_zombie_1448150".to_string(), 1448150, "python3".to_string()) => (set!{2}, 0.0, 0.0, 9383*1024),
                ("charlie".to_string(), 2233469, "python3".to_string())         => (set!{3}, 90.0, 23.0, 15771*1024)
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
                ("_zombie_3079002".to_string(), 3079002, "_unknown_".to_string()) => (HashSet::new(), 0.0, 0.0, 2350*1024)
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
            for ((user, pid, command), (cpu_percentage, cputime_sec, mem_percentage, mem_size)) in
                extract_ps_processes(&ps_output)
            {
                user_by_pid.insert(pid, user.clone());

                if (cpu_percentage >= cpu_cutoff_percent) || (mem_percentage >= mem_cutoff_percent)
                {
                    add_job_info(
                        &mut processes_by_job_id,
                        user,
                        jobs.job_id_from_pid(pid, &ps_output),
                        command,
                        cpu_percentage,
                        cputime_sec,
                        mem_size,
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
        // FIXME this does not print "none" or "unknown"
        let gpus_comma_separated: String = job_info
            .gpu_cards
            .iter()
            .map(|&num| num.to_string())
            .collect::<Vec<String>>()
            .join(",");

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
