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

#[derive(Clone)]
struct ProcInfo<'a> {
    user: &'a str,
    command: &'a str,
    _pid: Pid,
    rolledup: usize,
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
            rolledup: 0,
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
    rollup_by_jobid_and_command: bool,
    cpu_cutoff_percent: f64,
    mem_cutoff_percent: f64,
) {
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

    let timestamp = time_iso8601();
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let num_cores = num_cpus::get();
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let print_params = PrintParameters {
        hostname: &hostname,
        timestamp: &timestamp,
        num_cores,
        version: VERSION,
        cpu_cutoff_percent,
        mem_cutoff_percent
    };

    if rollup_by_jobid_and_command {
        // This is a little complicated because jobs with job_id 0 cannot be rolled up.
        //
        // - There is an array `rolledup` of ProcInfo nodes that represent rolled-up data
        //
        // - When the job ID of a job in `proc_by_pid` is zero, the entry in `rolledup` is a copy of
        //   that job; these jobs cannot be rolled up (this is why it's complicated)
        //
        // - Otherwise, the entry in `rolledup` represent rolled-up information for a (job, command) pair
        //
        // - There is a hash table `index` that maps the (job, command) pair to the entry in `rolledup`,
        //   if any
        //
        // - When we're done rolling up, we print the `rolledup` table.
        //
        // Filtering is performed after rolling up, so if a rolled-up job has a bunch of dinky
        // processes that together push it over the filtering limit then it will be printed.  This
        // is probably the right thing.

        let mut rolledup = vec![];
        let mut index = HashMap::<(JobID, &str), usize>::new();
        for (_, proc_info) in &proc_by_pid {
            if proc_info.job_id == 0 {
                rolledup.push(proc_info.clone());
            } else {
                let key = (proc_info.job_id, proc_info.command);
                if let Some(x) = index.get(&key) {
                    let p = &mut rolledup[*x];
                    p.cpu_percentage += proc_info.cpu_percentage;
                    p.cputime_sec += proc_info.cputime_sec;
                    p.mem_percentage += proc_info.mem_percentage;
                    p.mem_size_kib += proc_info.mem_size_kib;
                    p.gpu_cards.extend(&proc_info.gpu_cards);
                    p.gpu_percentage += proc_info.cpu_percentage;
                    p.gpu_mem_percentage += proc_info.gpu_mem_percentage;
                    p.gpu_mem_size_kib += proc_info.gpu_mem_size_kib;
                    p.rolledup += 1;
                } else {
                    let x = rolledup.len();
                    index.insert(key, x);
                    rolledup.push(proc_info.clone());
                    // We do not increment the clone's `rolledup` counter here because that counter
                    // counts how many *other* records have been rolled into the canonical one, 0
                    // means "no interesting information" and need not be printed.
                }
            }
        }

        for r in rolledup {
            print_record(&mut writer, &print_params, &r); 
        }
    } else {
        for (_, proc_info) in proc_by_pid {
            print_record(&mut writer, &print_params, &proc_info);
        }
    }

    writer.flush().unwrap();
}

struct PrintParameters<'a> {
    hostname: &'a str,
    timestamp: &'a str,
    num_cores: usize,
    version: &'a str,
    cpu_cutoff_percent: f64,
    mem_cutoff_percent: f64,
}

fn print_record<W: io::Write>(writer: &mut Writer<W>, params: &PrintParameters, proc_info: &ProcInfo) {
    if (proc_info.cpu_percentage < params.cpu_cutoff_percent) &&
        (proc_info.mem_percentage < params.mem_cutoff_percent) {
        return;
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
            &format!("v={}", params.version),
            &format!("time={}", params.timestamp),
            &format!("host={}", params.hostname),
            &format!("cores={}", params.num_cores),
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
            &format!("rolledup={}", proc_info.rolledup),
        ])
        .unwrap();
}
