use crate::amd;
use crate::gpu;
use crate::hostname;
use crate::log;
use crate::nvidia;
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
    let mut cards = if let Some(cs) = nvidia::get_nvidia_configuration() {
        cs
    } else if let Some(cs) = amd::get_amd_configuration() {
        cs
    } else {
        vec![]
    };
    let hostname = hostname::get().unwrap().into_string().unwrap();
    let ht = if threads_per_core > 1 {
        " (hyperthreaded)"
    } else {
        ""
    };
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
            while i < cards.len() && cards[i] == cards[first] {
                i += 1;
            }
            let memsize = if cards[first].mem_size_kib > 0 {
                (cards[first].mem_size_kib * 1024 / GIB as i64).to_string()
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
        (gpu_desc, gpu_cards, total_mem_by / GIB as i64)
    } else {
        ("".to_string(), 0, 0)
    };
    let timestamp = util::json_quote(timestamp);
    let hostname = util::json_quote(&hostname);
    let description = util::json_quote(&format!(
        "{sockets}x{cores_per_socket}{ht} {model}, {mem_gib} GiB{gpu_desc}"
    ));
    let cpu_cores = sockets * cores_per_socket * threads_per_core;

    // Note the field names here are used by decoders that are developed separately, and they should
    // be considered set in stone.

    let s = format!(
        r#"{{
  "timestamp": "{timestamp}",
  "hostname": "{hostname}",
  "description": "{description}",
  "cpu_cores": {cpu_cores},
  "mem_gb": {mem_gib},
  "gpu_cards": {gpu_cards},
  "gpumem_gb": {gpumem_gb}
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
