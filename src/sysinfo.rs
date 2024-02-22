use crate::amd;
use crate::nvidia;
use crate::gpu;
use crate::procfs;
use crate::procfsapi;

use serde::Serialize;

use std::io::{self, Write};

pub fn show_system(timestamp: &str) {
    let fs = procfsapi::RealFS::new();
    match do_show_system(&fs, timestamp) {
        Ok(_) => {}
        Err(e) => {
            println!("FAILED: {e}");
        }
    }
}

// It's possible this is not the right definition, it's possible we want 1000 * 1024 * 1024.

const GIGABYTE: usize = 1024 * 1024 * 1024;

// Note the field names here are used by decoders developed separately and should be considered set
// in stone.  All fields will be serialized; missing numeric values must be zero; consumers must
// deal with that.

#[derive(Serialize)]
struct NodeConfig {
    timestamp: String,
    hostname: String,
    description: String,
    cpu_cores: i32,
    mem_gb: i64,
    gpu_cards: i32,
    gpumem_gb: i64,
}

fn do_show_system(fs: &dyn procfsapi::ProcfsAPI, timestamp: &str) -> Result<(), String> {
    let (model, sockets, cores_per_socket, threads_per_core) = procfs::get_cpu_info(fs)?;
    let mem_by = procfs::get_memtotal_kib(fs)? * 1024;
    let mem_gb = (mem_by as f64 / GIGABYTE as f64).round() as i64;
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
    let (gpu_desc, gpu_cards, gpumem_gb) = if cards.len() > 0 {
        // Sort cards
        cards.sort_by(|a:&gpu::Card, b:&gpu::Card| {
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
                format!("{}", cards[first].mem_size_kib * 1024 / GIGABYTE as i64)
            } else {
                "unknown ".to_string()
            };
            gpu_desc += &format!(", {}x {} @ {}GB", (i - first), cards[first].model, memsize);
        }

        // Compute aggregate data
        let gpu_cards = cards.len() as i32;
        let mut total_mem_by = 0i64;
        for c in &cards {
            total_mem_by += c.mem_size_kib * 1024;
        }
        (gpu_desc, gpu_cards, total_mem_by / GIGABYTE as i64)
    } else {
        ("".to_string(), 0, 0)
    };
    let config = NodeConfig {
        timestamp: timestamp.to_string(),
        hostname,
        description: format!("{sockets}x{cores_per_socket}{ht} {model}, {mem_gb} GB{gpu_desc}"),
        cpu_cores: sockets * cores_per_socket * threads_per_core,
        mem_gb,
        gpu_cards,
        gpumem_gb,
    };
    match serde_json::to_string_pretty(&config) {
        Ok(s) => {
            let _ = io::stdout().write(s.as_bytes());
            let _ = io::stdout().write(b"\n");
            let _ = io::stdout().flush();
            Ok(())
        }
        Err(_) => Err("JSON encoding failed".to_string()),
    }
}
