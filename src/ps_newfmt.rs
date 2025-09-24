#![allow(clippy::comparison_to_empty)]

use crate::gpu;
use crate::json_tags::*;
use crate::output;
use crate::ps::{ProcInfo, PsOptions, SampleData};
use crate::systemapi;
use crate::util::three_places;

use std::collections::HashMap;

pub fn format_newfmt(
    c: &SampleData,
    system: &dyn systemapi::SystemAPI,
    opts: &PsOptions,
    recoverable_errors: output::Array,
) -> output::Object {
    let mut envelope = output::newfmt_envelope(system, opts.token.clone(), &[]);
    let (mut data, mut attrs) = output::newfmt_data(system, DATA_TAG_SAMPLE);
    attrs.push_s(SAMPLE_ATTRIBUTES_NODE, system.get_hostname());
    if opts.load {
        let mut sstate = output::Object::new();
        let mut cpu_load = output::Array::new();
        for v in &c.cpu_samples {
            cpu_load.push_i(*v as i64);
        }
        sstate.push_a(SAMPLE_SYSTEM_CPUS, cpu_load);
        if let Some(gpu_samples) = &c.gpu_samples {
            let mut gpu_load = output::Array::new();
            for v in gpu_samples {
                gpu_load.push_o(format_newfmt_gpu_sample(v));
            }
            sstate.push_a(SAMPLE_SYSTEM_GPUS, gpu_load);
        }
        sstate.push_u(SAMPLE_SYSTEM_USED_MEMORY, c.used_memory);
        sstate.push_f(SAMPLE_SYSTEM_LOAD1, c.load1);
        sstate.push_f(SAMPLE_SYSTEM_LOAD5, c.load5);
        sstate.push_f(SAMPLE_SYSTEM_LOAD15, c.load15);
        sstate.push_u(SAMPLE_SYSTEM_RUNNABLE_ENTITIES, c.runnable_entities);
        sstate.push_u(SAMPLE_SYSTEM_EXISTING_ENTITIES, c.existing_entities);
        attrs.push_o(SAMPLE_ATTRIBUTES_SYSTEM, sstate);
    }
    // Group processes under (user, jobid) except for jobid 0.
    // `collected` collects the sample indices for like (user,job) where job != 0.
    // `zeroes` collects the sample indices for job = 0.
    let mut collected = HashMap::<(&str, usize), Vec<usize>>::new();
    let mut zeroes = vec![];
    for i in 0..c.process_samples.len() {
        let sample = &c.process_samples[i];
        if sample.job_id == 0 {
            zeroes.push(i);
        } else {
            collected
                .entry((&sample.user, sample.job_id))
                .and_modify(|e| e.push(i))
                .or_insert(vec![i]);
        }
    }
    let mut jobs = output::Array::new();
    for k in zeroes {
        let j = &c.process_samples[k];
        jobs.push_o(format_newfmt_job(
            system,
            0,
            &j.user,
            &[k],
            &c.process_samples,
        ));
    }
    for ((user, id), ixs) in collected {
        jobs.push_o(format_newfmt_job(
            system,
            id,
            user,
            &ixs,
            &c.process_samples,
        ));
    }
    attrs.push_a(SAMPLE_ATTRIBUTES_JOBS, jobs);
    if !recoverable_errors.is_empty() {
        attrs.push_a(SAMPLE_ATTRIBUTES_ERRORS, recoverable_errors);
    }
    data.push_o(SAMPLE_DATA_ATTRIBUTES, attrs);
    envelope.push_o(SAMPLE_ENVELOPE_DATA, data);
    envelope
}

fn format_newfmt_gpu_sample(c: &gpu::CardState) -> output::Object {
    let mut s = output::Object::new();
    if c.device.index != 0 {
        s.push_i(SAMPLE_GPU_INDEX, c.device.index as i64);
    }
    if c.device.uuid != "" {
        s.push_s(SAMPLE_GPU_UUID, c.device.uuid.clone());
    }
    if c.failing != 0 {
        s.push_i(SAMPLE_GPU_FAILING, c.failing as i64);
    }
    if c.fan_speed_pct != 0.0 {
        s.push_i(SAMPLE_GPU_FAN, c.fan_speed_pct.round() as i64);
    }
    if c.compute_mode != "" {
        s.push_s(SAMPLE_GPU_COMPUTE_MODE, c.compute_mode.clone());
    }
    let perf = (c.perf_state + 1) as u64; // extended-unsigned encoding, perf_state may be -1 here.
    if perf != 0 {
        s.push_u(SAMPLE_GPU_PERFORMANCE_STATE, perf);
    }
    if c.mem_used_kib != 0 {
        s.push_u(SAMPLE_GPU_MEMORY, c.mem_used_kib);
    }
    if c.gpu_utilization_pct != 0.0 {
        s.push_i(SAMPLE_GPU_CEUTIL, c.gpu_utilization_pct.round() as i64);
    }
    if c.mem_utilization_pct != 0.0 {
        s.push_i(SAMPLE_GPU_MEMORY_UTIL, c.mem_utilization_pct.round() as i64);
    }
    if c.temp_c != 0 {
        s.push_i(SAMPLE_GPU_TEMPERATURE, c.temp_c as i64);
    }
    if c.power_watt != 0 {
        s.push_i(SAMPLE_GPU_POWER, c.power_watt as i64);
    }
    if c.power_limit_watt != 0 {
        s.push_i(SAMPLE_GPU_POWER_LIMIT, c.power_limit_watt as i64);
    }
    if c.ce_clock_mhz != 0 {
        s.push_i(SAMPLE_GPU_CECLOCK, c.ce_clock_mhz as i64);
    }
    if c.mem_clock_mhz != 0 {
        s.push_i(SAMPLE_GPU_MEMORY_CLOCK, c.mem_clock_mhz as i64);
    }
    s
}

fn format_newfmt_job(
    system: &dyn systemapi::SystemAPI,
    id: usize,
    user: &str,
    ixs: &[usize], // Not empty
    samples: &[ProcInfo],
) -> output::Object {
    let mut job = output::Object::new();
    job.push_u(SAMPLE_JOB_JOB, id as u64);
    job.push_s(SAMPLE_JOB_USER, user.to_string());
    if !samples[ixs[0]].is_slurm {
        // Every sample in the job is either slurm or not, so it's enough to check the first.
        job.push_u(
            SAMPLE_JOB_EPOCH,
            system.get_boot_time_in_secs_since_epoch() - EPOCH_TIME_BASE,
        );
    }
    let mut procs = output::Array::new();
    for ix in ixs {
        procs.push_o(format_newfmt_sample(&samples[*ix]));
    }
    job.push_a(SAMPLE_JOB_PROCESSES, procs);
    job
}

fn format_newfmt_sample(proc_info: &ProcInfo) -> output::Object {
    let mut fields = output::Object::new();

    if proc_info.rssanon_kib != 0 {
        fields.push_u(SAMPLE_PROCESS_RESIDENT_MEMORY, proc_info.rssanon_kib as u64);
    }
    if proc_info.mem_size_kib != 0 {
        fields.push_u(SAMPLE_PROCESS_VIRTUAL_MEMORY, proc_info.mem_size_kib as u64);
    }
    if proc_info.data_read_kib != 0 {
        fields.push_u(SAMPLE_PROCESS_READ, proc_info.data_read_kib as u64);
    }
    if proc_info.data_written_kib != 0 {
        fields.push_u(SAMPLE_PROCESS_WRITTEN, proc_info.data_written_kib as u64);
    }
    if proc_info.data_cancelled_kib != 0 {
        fields.push_u(
            SAMPLE_PROCESS_CANCELLED,
            proc_info.data_cancelled_kib as u64,
        );
    }
    fields.push_s(SAMPLE_PROCESS_CMD, proc_info.command.to_string());
    if proc_info.rolledup == 0 && proc_info.pid != 0 {
        // pid must be 0 for rolledup > 0 as there is no guarantee that there is any fixed
        // representative pid for a rolled-up set of processes: the set can change from run to run,
        // and sonar has no history.
        fields.push_u(SAMPLE_PROCESS_PID, proc_info.pid as u64);
    }
    if proc_info.ppid != 0 {
        fields.push_u(SAMPLE_PROCESS_PARENT_PID, proc_info.ppid as u64);
    }
    if proc_info.num_threads != 0 {
        fields.push_u(SAMPLE_PROCESS_NUM_THREADS, proc_info.num_threads as u64);
    }
    if proc_info.cpu_percentage != 0.0 {
        fields.push_f(
            SAMPLE_PROCESS_CPU_AVG,
            three_places(proc_info.cpu_percentage),
        );
    }
    if proc_info.cpu_util != 0.0 {
        fields.push_f(SAMPLE_PROCESS_CPU_UTIL, three_places(proc_info.cpu_util));
    }
    if proc_info.cputime_sec != 0 {
        fields.push_u(SAMPLE_PROCESS_CPU_TIME, proc_info.cputime_sec as u64);
    }
    if proc_info.rolledup > 0 {
        fields.push_u(SAMPLE_PROCESS_ROLLEDUP, proc_info.rolledup as u64);
    }
    if !proc_info.gpus.is_empty() {
        let mut gpus = output::Array::new();
        for g in proc_info.gpus.values() {
            let mut gpu = output::Object::new();
            gpu.push_u(SAMPLE_PROCESS_GPU_INDEX, g.device.index as u64);
            gpu.push_s(SAMPLE_PROCESS_GPU_UUID, g.device.uuid.clone());
            if g.gpu_util != 0 {
                gpu.push_u(SAMPLE_PROCESS_GPU_GPU_UTIL, g.gpu_util as u64);
            }
            if g.gpu_mem != 0 {
                gpu.push_u(SAMPLE_PROCESS_GPU_GPU_MEMORY, g.gpu_mem);
            }
            if g.gpu_mem_util != 0 {
                gpu.push_u(SAMPLE_PROCESS_GPU_GPU_MEMORY_UTIL, g.gpu_mem_util as u64);
            }
            gpus.push_o(gpu);
        }
        fields.push_a(SAMPLE_PROCESS_GPUS, gpus);
    }

    fields
}
