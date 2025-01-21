use crate::gpu;
use crate::hostname;
use crate::log;
use crate::procfs;
use crate::procfsapi;
use crate::util;

use std::io;

pub fn show_system(timestamp: &str) {
    let fs = procfsapi::RealFS::new();
    let mut writer = io::stdout();
    match do_show_system(&mut writer, &fs, timestamp) {
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
    let (gpu_desc, gpu_cards, gpumem_gb, gpu_info) = if !cards.is_empty() {
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
        let mut gpu_info = "".to_string();
        for c in &cards {
            if !gpu_info.is_empty() {
                gpu_info += ","
            }
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
            let manufacturer = util::json_quote(&manufacturer);
            let bus_addr = util::json_quote(bus_addr);
            let model = util::json_quote(model);
            let arch = util::json_quote(arch);
            let driver = util::json_quote(driver);
            let firmware = util::json_quote(firmware);
            gpu_info += &format!(
                r###"
  {{"bus_addr":"{bus_addr}", "index":{index}, "uuid":"{uuid}",
   "manufacturer":"{manufacturer}", "model":"{model}", "arch":"{arch}", "driver":"{driver}", "firmware":"{firmware}",
   "mem_size_kib":{mem_size_kib},
   "power_limit_watt":{power_limit_watt}, "max_power_limit_watt":{max_power_limit_watt}, "min_power_limit_watt":{min_power_limit_watt},
   "max_ce_clock_mhz":{max_ce_clock_mhz}, "max_mem_clock_mhz":{max_mem_clock_mhz}}}"###
            );
        }

        (gpu_desc, gpu_cards, total_mem_by / GIB as i64, gpu_info)
    } else {
        ("".to_string(), 0, 0, "".to_string())
    };
    let timestamp = util::json_quote(timestamp);
    let hostname = util::json_quote(&hostname);
    let description = util::json_quote(&format!(
        "{sockets}x{cores_per_socket}{ht} {model}, {mem_gib} GiB{gpu_desc}"
    ));
    let cpu_cores = sockets * cores_per_socket * threads_per_core;

    // Note the field names here are used by decoders that are developed separately, and they should
    // be considered set in stone.

    let version = util::json_quote(env!("CARGO_PKG_VERSION"));
    let s = format!(
        r#"{{
  "version": "{version}",
  "timestamp": "{timestamp}",
  "hostname": "{hostname}",
  "description": "{description}",
  "cpu_cores": {cpu_cores},
  "mem_gb": {mem_gib},
  "gpu_cards": {gpu_cards},
  "gpumem_gb": {gpumem_gb},
  "gpu_info": [{gpu_info}]
}}
"#
    );

    // Ignore I/O errors.

    let _ = writer.write(s.as_bytes());
    let _ = writer.flush();
    Ok(())
}

// Currently the test for do_show_system() is black-box, see ../tests.  The reason for this is partly
// that not all the system interfaces used by that function are virtualized at this time, and partly
// that we only care that the output syntax looks right.
