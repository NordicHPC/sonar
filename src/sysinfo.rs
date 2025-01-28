use crate::gpu;
use crate::hostname;
use crate::log;
use crate::output;
use crate::procfs;
use crate::procfsapi;

use std::io;

pub fn show_system(writer: &mut dyn io::Write, timestamp: &str, csv: bool) {
    let fs = procfsapi::RealFS::new();
    match do_show_system(writer, &fs, timestamp, csv) {
        Ok(_) => {}
        Err(e) => {
            log::error(&format!("sysinfo failed: {e}"));
        }
    }
}

const GIB: usize = 1024 * 1024 * 1024;

fn do_show_system(
    writer: &mut dyn io::Write,
    fs: &dyn procfsapi::ProcfsAPI,
    timestamp: &str,
    csv: bool,
) -> Result<(), String> {
    let (model, sockets, cores_per_socket, threads_per_core) = procfs::get_cpu_info(fs)?;
    let mem_by = procfs::get_memtotal_kib(fs)? * 1024;
    let mem_gib = (mem_by as f64 / GIB as f64).round() as i64;
    let (mut cards, manufacturer) = match gpu::probe() {
        Some(mut device) => (
            device.get_card_configuration().unwrap_or_default(),
            device.get_manufacturer(),
        ),
        None => (vec![], "UNKNOWN".to_string()),
    };
    let hostname = hostname::get();
    let ht = if threads_per_core > 1 {
        " (hyperthreaded)"
    } else {
        ""
    };

    let mut gpu_info = output::Array::new();
    let (gpu_desc, gpu_cards, gpumem_gb) = if !cards.is_empty() {
        // Sort cards
        cards.sort_by(|a: &gpu::Card, b: &gpu::Card| {
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
                ((cards[first].mem_size_kib as f64 * 1024.0 / GIB as f64).round() as usize).to_string()
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
            let gpu::Card {
                bus_addr,
                index,
                model,
                arch,
                driver,
                firmware,
                uuid,
                mem_size_kib,
                power_limit_watt,
                max_power_limit_watt,
                min_power_limit_watt,
                max_ce_clock_mhz,
                max_mem_clock_mhz,
            } = c;
            let mut gpu = output::Object::new();
            gpu.push_s("bus_addr", bus_addr.to_string());
            gpu.push_i("index", *index as i64);
            gpu.push_s("uuid", uuid.to_string());
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

    let mut sysinfo = output::Object::new();
    sysinfo.push_s("version", env!("CARGO_PKG_VERSION").to_string());
    sysinfo.push_s("timestamp", timestamp.to_string());
    sysinfo.push_s("hostname", hostname);
    sysinfo.push_s(
        "description",
        format!("{sockets}x{cores_per_socket}{ht} {model}, {mem_gib} GiB{gpu_desc}"),
    );
    sysinfo.push_i("cpu_cores", cpu_cores as i64);
    sysinfo.push_i("mem_gb", mem_gib);
    sysinfo.push_i("gpu_cards", gpu_cards as i64);
    sysinfo.push_i("gpumem_gb", gpumem_gb);
    if gpu_info.len() > 0 {
        sysinfo.push_a("gpu_info", gpu_info);
    }

    if csv {
        output::write_csv(writer, &output::Value::O(sysinfo));
    } else {
        output::write_json(writer, &output::Value::O(sysinfo));
    }
    Ok(())
}

// Currently the test for do_show_system() is black-box, see ../tests.  The reason for this is partly
// that not all the system interfaces used by that function are virtualized at this time, and partly
// that we only care that the output syntax looks right.
