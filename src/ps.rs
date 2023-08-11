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

type GpuSet = HashSet<usize>;

fn make_gpuset(maybe_device: Option<usize>) -> GpuSet {
    let mut gpus = GpuSet::new();
    if let Some(dev) = maybe_device {
        gpus.insert(dev);
    }
    gpus
}

type Pid = usize;
type JobID = usize;

// ProcInfo holds per-process information gathered from multiple sources and tagged with a job ID.
// No processes are merged!  The job ID "0" means "unique job with no job ID".  That is, no consumer
// of this data, internal or external to the program, may treat processes with job ID "0" as part of
// the same job.

struct ProcInfo<'a> {
    user: &'a str,
    command: &'a str,
    _pid: Pid,
    job_id: usize,
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_percentage: f64,
    mem_size_kib: usize,
    gpu_cards: GpuSet,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size_kib: usize,
}

type ProcTable<'a> = HashMap<Pid, ProcInfo<'a>>;

// Add information about the process to the table `proc_by_pid`.  Here, `lookup_job_by_pid`, `user`,
// `command`, and `pid` must be provided while the subsequent fields are all optional and must be
// zero / empty if there's no information.

fn add_proc_info<'a, F>(
    proc_by_pid: &mut ProcTable<'a>,
    lookup_job_by_pid: &mut F,
    user: &'a str,
    command: &'a str,
    pid: Pid,
    cpu_percentage: f64,
    cputime_sec: usize,
    mem_percentage: f64,
    mem_size_kib: usize,
    gpu_cards: &GpuSet,
    gpu_percentage: f64,
    gpu_mem_percentage: f64,
    gpu_mem_size_kib: usize,
)
where
    F: FnMut(Pid) -> JobID
{
    proc_by_pid
        .entry(pid)
        .and_modify(|e| {
            // Already has user, command, pid, job_id
            e.cpu_percentage += cpu_percentage;
            e.cputime_sec += cputime_sec;
            e.mem_percentage += mem_percentage;
            e.mem_size_kib += mem_size_kib;
            e.gpu_cards.extend(gpu_cards);
            e.gpu_percentage += gpu_percentage;
            e.gpu_mem_percentage += gpu_mem_percentage;
            e.gpu_mem_size_kib += gpu_mem_size_kib;
        })
        .or_insert(ProcInfo {
            user,
            command,
            _pid: pid,
            job_id: lookup_job_by_pid(pid),
            cpu_percentage,
            cputime_sec,
            mem_percentage,
            mem_size_kib,
            gpu_cards: gpu_cards.clone(),
            gpu_percentage,
            gpu_mem_percentage,
            gpu_mem_size_kib,
        });
}

pub fn create_snapshot(
    jobs: &mut dyn jobs::JobManager,
    cpu_cutoff_percent: f64,
    mem_cutoff_percent: f64,
) {
    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();
    let no_gpus = make_gpuset(None);
    let mut proc_by_pid = ProcTable::new();

    let ps_probe = process::get_process_information(jobs);
    if let Err(e) = ps_probe {
        // This is a hard error, we need this information for everything.
        log::error!("CPU process listing failed: {:?}", e);
        return;
    }
    let ps_output = &ps_probe.unwrap();

    // The table of users is needed to get GPU information
    let mut user_by_pid: HashMap<usize, String> = HashMap::new();
    for proc in ps_output {
        user_by_pid.insert(proc.pid, proc.user.clone());
    }

    let mut lookup_job_by_pid = |pid: Pid| {
        jobs.job_id_from_pid(pid, ps_output)
    };

    for proc in ps_output {
        add_proc_info(&mut proc_by_pid,
                      &mut lookup_job_by_pid,
                      &proc.user,
                      &proc.command,
                      proc.pid,
                      proc.cpu_pct,
                      proc.cputime_sec,
                      proc.mem_pct,
                      proc.mem_size_kib,
                      &no_gpus, // gpu_cards
                      0.0,      // gpu_percentage
                      0.0,      // gpu_mem_percentage
                      0);       // gpu_mem_size_kib
    }

    let nvidia_probe = nvidia::get_nvidia_information(&user_by_pid);
    match nvidia_probe {
        Err(e) => {
            // This is a soft error.
            log::error!("GPU (Nvidia) process listing failed: {:?}", e);
        }
        Ok(ref nvidia_output) => {
            for proc in nvidia_output {
                add_proc_info(&mut proc_by_pid,
                              &mut lookup_job_by_pid,
                              &proc.user,
                              &proc.command,
                              proc.pid,
                              0.0, // cpu_percentage
                              0,   // cputime_sec
                              0.0, // mem_percentage
                              0,   // mem_size_kib
                              &make_gpuset(proc.device),
                              proc.gpu_pct,
                              proc.mem_pct,
                              proc.mem_size_kib);
            }
        }
    }

    let amd_probe = amd::get_amd_information(&user_by_pid);
    match amd_probe {
        Err(e) => {
            // This is a soft error.
            log::error!("GPU (Nvidia) process listing failed: {:?}", e);
        }
        Ok(ref amd_output) => {
            for proc in amd_output {
                add_proc_info(&mut proc_by_pid,
                              &mut lookup_job_by_pid,
                              &proc.user,
                              &proc.command,
                              proc.pid,
                              0.0, // cpu_percentage
                              0,   // cputime_sec
                              0.0, // mem_percentage
                              0,   // mem_size_kib
                              &make_gpuset(proc.device),
                              proc.gpu_pct,
                              proc.mem_pct,
                              proc.mem_size_kib);
            }
        }
    }

    let mut writer = Writer::from_writer(io::stdout());

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    for (_, proc_info) in proc_by_pid {
        if (proc_info.cpu_percentage < cpu_cutoff_percent) && (proc_info.mem_percentage < mem_cutoff_percent) {
            continue;
        }
        // "unknown" is not implemented, see https://github.com/NordicHPC/sonar/issues/75
        let mut gpus_comma_separated: String = proc_info
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
                &format!("user={}", proc_info.user),
                &format!("job={}", proc_info.job_id),
                &format!("cmd={}", proc_info.command),
                &format!("cpu%={}", three_places(proc_info.cpu_percentage)),
                &format!("cpukib={}", proc_info.mem_size_kib),
                &format!("gpus={}", gpus_comma_separated),
                &format!("gpu%={}", three_places(proc_info.gpu_percentage)),
                &format!("gpumem%={}", three_places(proc_info.gpu_mem_percentage)),
                &format!("gpukib={}", proc_info.gpu_mem_size_kib),
                &format!("cputime_sec={}", proc_info.cputime_sec),
            ])
            .unwrap();
    }

    writer.flush().unwrap();
}
