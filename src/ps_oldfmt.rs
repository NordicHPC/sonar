#![allow(clippy::len_zero)]
#![allow(clippy::comparison_to_empty)]

use crate::gpuapi;
use crate::json_tags;
use crate::output;
use crate::ps::{GpuStatus, ProcInfo, PsOptions, SampleData};
use crate::systemapi;
use crate::util::three_places;

pub fn make_oldfmt_heartbeat(system: &dyn systemapi::SystemAPI) -> output::Object {
    let mut fields = output::Object::new();
    fields.push_s("v", system.get_version());
    fields.push_s("time", system.get_timestamp());
    fields.push_s("host", system.get_hostname());
    fields.push_s("user", "_sonar_".to_string());
    fields.push_s("cmd", "_heartbeat_".to_string());
    fields
}

pub fn format_oldfmt(
    c: &SampleData,
    system: &dyn systemapi::SystemAPI,
    opts: &PsOptions,
) -> output::Array {
    let mut records = vec![];
    for p in &c.process_samples {
        records.push(format_oldfmt_sample(p, system));
    }

    if opts.load && records.len() > 0 {
        if !c.cpu_samples.is_empty() {
            let mut a = output::Array::from_vec(
                c.cpu_samples
                    .iter()
                    .map(|x| output::Value::U(*x))
                    .collect::<Vec<output::Value>>(),
            );
            a.set_encode_nonempty_base45();
            records[0].push_a("load", a);
        }
        if let Some(samples) = &c.gpu_samples {
            if let Some(formatted) = format_gpu_samples_horizontally(samples) {
                records[0].push_o("gpuinfo", formatted);
            }
        }
    }

    let mut result = output::Array::new();
    for v in records {
        result.push_o(v);
    }
    result
}

fn format_oldfmt_sample(proc_info: &ProcInfo, system: &dyn systemapi::SystemAPI) -> output::Object {
    let mut fields = output::Object::new();

    fields.push_s("v", system.get_version());
    fields.push_s("time", system.get_timestamp());
    fields.push_s("host", system.get_hostname());
    fields.push_s("user", proc_info.user.to_string());
    fields.push_s("cmd", proc_info.command.to_string());

    // Only print optional fields whose values are not their defaults.  The defaults are defined in
    // README.md.  The values there must agree with those used by Jobanalyzer's parser.

    if proc_info.job_id != 0 {
        fields.push_u("job", proc_info.job_id as u64);
    }
    if !proc_info.is_slurm {
        fields.push_u("epoch", system.get_boot_time() - json_tags::EPOCH_TIME_BASE);
    }
    if proc_info.rolledup == 0 && proc_info.pid != 0 {
        // pid must be 0 for rolledup > 0 as there is no guarantee that there is any fixed
        // representative pid for a rolled-up set of processes: the set can change from run to run,
        // and sonar has no history.
        fields.push_u("pid", proc_info.pid as u64);
    }
    if proc_info.ppid != 0 {
        fields.push_u("ppid", proc_info.ppid as u64);
    }
    if proc_info.cpu_percentage != 0.0 {
        fields.push_f("cpu%", three_places(proc_info.cpu_percentage));
    }
    if proc_info.mem_size_kib != 0 {
        fields.push_u("cpukib", proc_info.mem_size_kib as u64);
    }
    if proc_info.rssanon_kib != 0 {
        fields.push_u("rssanonkib", proc_info.rssanon_kib as u64);
    }
    if proc_info.gpus.is_empty() {
        // Nothing
    } else {
        fields.push_s(
            "gpus",
            proc_info
                .gpus
                .keys()
                .map(|device| device.index.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    }
    if proc_info.gpu_percentage != 0.0 {
        fields.push_f("gpu%", three_places(proc_info.gpu_percentage));
    }
    if proc_info.gpu_mem_percentage != 0.0 {
        fields.push_f("gpumem%", three_places(proc_info.gpu_mem_percentage));
    }
    if proc_info.gpu_mem_size_kib != 0 {
        fields.push_u("gpukib", proc_info.gpu_mem_size_kib as u64);
    }
    if proc_info.cputime_sec != 0 {
        fields.push_u("cputime_sec", proc_info.cputime_sec as u64);
    }
    if proc_info.gpu_status != GpuStatus::Ok {
        fields.push_u("gpufail", proc_info.gpu_status as u64);
    }
    if proc_info.rolledup > 0 {
        fields.push_u("rolledup", proc_info.rolledup as u64);
    }

    fields
}

// This creates the sequence-of-attributes encoding for per-node GPU data for the old CSV format.

fn format_gpu_samples_horizontally(cards: &[gpuapi::CardState]) -> Option<output::Object> {
    let mut s = output::Object::new();
    s = add_key(s, "fan%", cards, |c: &gpuapi::CardState| {
        nonzero(c.fan_speed_pct as i64)
    });
    s = add_key(s, "mode", cards, |c: &gpuapi::CardState| {
        if c.compute_mode == "" {
            output::Value::E()
        } else {
            output::Value::S(c.compute_mode.clone())
        }
    });
    s = add_key(s, "perf", cards, |c: &gpuapi::CardState| {
        output::Value::S(if c.perf_state == -1 {
            "".to_string()
        } else {
            format!("P{}", c.perf_state)
        })
    });
    // Reserved memory is really not interesting, it's possible it would have been
    // interesting as part of the card configuration.
    //s = add_key(s, "mreskib", cards, |c: &gpuapi::CardState| nonzero(c.mem_reserved_kib));
    s = add_key(s, "musekib", cards, |c: &gpuapi::CardState| {
        nonzero(c.mem_used_kib as i64)
    });
    s = add_key(s, "cutil%", cards, |c: &gpuapi::CardState| {
        nonzero(c.gpu_utilization_pct as i64)
    });
    s = add_key(s, "mutil%", cards, |c: &gpuapi::CardState| {
        nonzero(c.mem_utilization_pct as i64)
    });
    s = add_key(s, "tempc", cards, |c: &gpuapi::CardState| {
        nonzero(c.temp_c.into())
    });
    s = add_key(s, "poww", cards, |c: &gpuapi::CardState| {
        nonzero(c.power_watt.into())
    });
    s = add_key(s, "powlimw", cards, |c: &gpuapi::CardState| {
        nonzero(c.power_limit_watt.into())
    });
    s = add_key(s, "cez", cards, |c: &gpuapi::CardState| {
        nonzero(c.ce_clock_mhz.into())
    });
    s = add_key(s, "memz", cards, |c: &gpuapi::CardState| {
        nonzero(c.mem_clock_mhz.into())
    });
    if !s.is_empty() {
        Some(s)
    } else {
        None
    }
}

fn add_key(
    mut s: output::Object,
    key: &str,
    cards: &[gpuapi::CardState],
    extract: fn(&gpuapi::CardState) -> output::Value,
) -> output::Object {
    let mut vs = output::Array::new();
    let mut any_nonempty = false;
    vs.set_csv_separator("|".to_string());
    for c in cards {
        let v = extract(c);
        if let output::Value::E() = v {
        } else {
            any_nonempty = true;
        }
        vs.push(v);
    }
    if any_nonempty {
        s.push(key, output::Value::A(vs));
    }
    s
}

fn nonzero(x: i64) -> output::Value {
    if x == 0 {
        output::Value::E()
    } else {
        output::Value::I(x)
    }
}
