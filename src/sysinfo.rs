use crate::gpuapi;
use crate::output;
use crate::procfs;
use crate::systemapi;

use std::io;

pub fn show_system(writer: &mut dyn io::Write, system: &dyn systemapi::SystemAPI, csv: bool) {
    let sysinfo = compute_sysinfo(system);
    if csv {
        output::write_csv(writer, &output::Value::O(sysinfo));
    } else {
        output::write_json(writer, &output::Value::O(sysinfo));
    }
}

// The packet always has "version", "timestamp", and "hostname", and then it has either an "error"
// field or the sysinfo fields ("cpu_cores", etc) for the node.  Fields that have default values (0,
// "", []) may be omitted.

pub fn compute_sysinfo(system: &dyn systemapi::SystemAPI) -> output::Object {
    try_compute_sysinfo(system).unwrap_or_else(|e: String| error_packet(system, e))
}

const GIB: usize = 1024 * 1024 * 1024;

fn try_compute_sysinfo(system: &dyn systemapi::SystemAPI) -> Result<output::Object, String> {
    let fs = system.get_procfs();
    let gpus = system.get_gpus();
    let procfs::CpuInfo { cores, sockets, cores_per_socket, threads_per_core } =
        procfs::get_cpu_info(fs)?;
    let model = &cores[0].model_name; // expedient: normally all cores are the same
    let mem_by = procfs::get_memtotal_kib(fs)? * 1024;
    let mem_gib = (mem_by as f64 / GIB as f64).round() as i64;
    let (mut cards, manufacturer) = match gpus.probe() {
        Some(device) => (
            device.get_card_configuration().unwrap_or_default(),
            device.get_manufacturer(),
        ),
        None => (vec![], "UNKNOWN".to_string()),
    };
    let ht = if threads_per_core > 1 {
        " (hyperthreaded)"
    } else {
        ""
    };

    let mut gpu_info = output::Array::new();
    let (gpu_desc, gpu_cards, gpumem_gb) = if !cards.is_empty() {
        // Sort cards
        cards.sort_by(|a: &gpuapi::Card, b: &gpuapi::Card| {
            if a.model == b.model {
                a.mem_size_kib.cmp(&b.mem_size_kib)
            } else {
                a.model.cmp(&b.model)
            }
        });

        // Merge equal cards
        let mut i = 0;
        let mut gpu_desc = "".to_string();
        while i < cards.len() {
            let first = i;
            while i < cards.len()
                && cards[i].model == cards[first].model
                && cards[i].mem_size_kib == cards[first].mem_size_kib
            {
                i += 1;
            }
            let memsize = if cards[first].mem_size_kib > 0 {
                ((cards[first].mem_size_kib as f64 * 1024.0 / GIB as f64).round() as usize)
                    .to_string()
            } else {
                "unknown ".to_string()
            };
            gpu_desc += &format!(", {}x {} @ {}GiB", (i - first), cards[first].model, memsize);
        }

        // Compute aggregate data
        let gpu_cards = cards.len() as i32;
        let mut total_mem_by = 0i64;
        for c in &cards {
            total_mem_by += c.mem_size_kib * 1024;
        }

        // Compute the info blobs
        for c in &cards {
            let gpuapi::Card {
                bus_addr,
                device,
                model,
                arch,
                driver,
                firmware,
                mem_size_kib,
                power_limit_watt,
                max_power_limit_watt,
                min_power_limit_watt,
                max_ce_clock_mhz,
                max_mem_clock_mhz,
            } = c;
            let mut gpu = output::Object::new();
            gpu.push_s("bus_addr", bus_addr.to_string());
            gpu.push_i("index", device.index as i64);
            gpu.push_s("uuid", device.uuid.to_string());
            gpu.push_s("manufacturer", manufacturer.clone());
            gpu.push_s("model", model.to_string());
            gpu.push_s("arch", arch.to_string());
            gpu.push_s("driver", driver.to_string());
            gpu.push_s("firmware", firmware.to_string());
            gpu.push_i("mem_size_kib", *mem_size_kib);
            gpu.push_i("power_limit_watt", *power_limit_watt as i64);
            gpu.push_i("max_power_limit_watt", *max_power_limit_watt as i64);
            gpu.push_i("min_power_limit_watt", *min_power_limit_watt as i64);
            gpu.push_i("max_ce_clock_mhz", *max_ce_clock_mhz as i64);
            gpu.push_i("max_mem_clock_mhz", *max_mem_clock_mhz as i64);
            gpu_info.push_o(gpu);
        }

        (gpu_desc, gpu_cards, total_mem_by / GIB as i64)
    } else {
        ("".to_string(), 0, 0)
    };
    let cpu_cores = sockets * cores_per_socket * threads_per_core;

    let mut sysinfo = new_sysinfo(system);
    sysinfo.push_s(
        "description",
        format!("{sockets}x{cores_per_socket}{ht} {model}, {mem_gib} GiB{gpu_desc}"),
    );
    sysinfo.push_i("cpu_cores", cpu_cores as i64);
    sysinfo.push_i("mem_gb", mem_gib);
    if gpu_cards != 0 {
        sysinfo.push_i("gpu_cards", gpu_cards as i64);
        if gpumem_gb != 0 {
            sysinfo.push_i("gpumem_gb", gpumem_gb);
        }
        if gpu_info.len() > 0 {
            sysinfo.push_a("gpu_info", gpu_info);
        }
    }

    Ok(sysinfo)
}

fn error_packet(system: &dyn systemapi::SystemAPI, error: String) -> output::Object {
    let mut sysinfo = new_sysinfo(system);
    sysinfo.push_s("error", error);
    sysinfo
}

fn new_sysinfo(system: &dyn systemapi::SystemAPI) -> output::Object {
    let mut sysinfo = output::Object::new();
    sysinfo.push_s("version", system.get_version());
    sysinfo.push_s("timestamp", system.get_timestamp());
    sysinfo.push_s("hostname", system.get_hostname());
    return sysinfo;
}
