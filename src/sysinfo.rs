#![allow(clippy::len_zero)]
#![allow(clippy::comparison_to_empty)]

use crate::command;
use crate::gpu;
use crate::json_tags::*;
use crate::output;
use crate::systemapi;

use base64::{engine::general_purpose::STANDARD, Engine as _};

use std::io;

#[cfg(feature = "daemon")]
pub struct State<'a> {
    system: &'a dyn systemapi::SystemAPI,
    topo_svg_cmd: Option<String>,
    topo_text_cmd: Option<String>,
    token: String,
}

#[cfg(feature = "daemon")]
impl<'a> State<'a> {
    pub fn new(
        system: &'a dyn systemapi::SystemAPI,
        topo_svg_cmd: Option<String>,
        topo_text_cmd: Option<String>,
        token: String,
    ) -> State<'a> {
        State {
            system,
            topo_svg_cmd,
            topo_text_cmd,
            token,
        }
    }

    pub fn run(&mut self) -> Vec<Vec<u8>> {
        let mut writer = Vec::new();
        show_system(
            &mut writer,
            self.system,
            self.token.clone(),
            Format::NewJSON,
            self.topo_svg_cmd.clone(),
            self.topo_text_cmd.clone(),
        );
        vec![writer]
    }
}

#[derive(Clone)]
pub enum Format {
    CSV,
    OldJSON,
    NewJSON,
}

pub fn show_system(
    writer: &mut dyn io::Write,
    system: &dyn systemapi::SystemAPI,
    token: String,
    fmt: Format,
    topo_svg_cmd: Option<String>,
    topo_text_cmd: Option<String>,
) {
    let sysinfo = match compute_nodeinfo(system) {
        Ok(mut info) => {
            if let Some(cmd) = topo_svg_cmd {
                if let Some(output) = run_command_unsafely(cmd) {
                    info.topo_svg = Some(output);
                }
            }
            if let Some(cmd) = topo_text_cmd {
                if let Some(output) = run_command_unsafely(cmd) {
                    info.topo_text = Some(output);
                }
            }
            match fmt {
                Format::NewJSON => layout_sysinfo_newfmt(system, token, info),
                Format::CSV | Format::OldJSON => layout_sysinfo_oldfmt(system, info),
            }
        }
        Err(e) => match fmt {
            Format::NewJSON => layout_error_newfmt(system, token, e),
            Format::CSV | Format::OldJSON => layout_error_oldfmt(system, e),
        },
    };
    match fmt {
        Format::CSV => {
            output::write_csv(writer, &output::Value::O(sysinfo));
        }
        Format::NewJSON | Format::OldJSON => {
            output::write_json(writer, &output::Value::O(sysinfo));
        }
    }
}

// "Unsafely" because technically both the verb and args can contain spaces, but there's no way to
// express that.
fn run_command_unsafely(cmd: String) -> Option<String> {
    let mut tokens = cmd.split_ascii_whitespace();
    match tokens.next() {
        Some(verb) => {
            let args = tokens.collect::<Vec<&str>>();
            match command::safe_command(verb, &args, 5) {
                Ok((s, _)) => Some(s),
                Err(_) => None,
            }
        }
        None => None,
    }
}

// New JSON format - json:api compatible, see spec.

fn layout_sysinfo_newfmt(
    system: &dyn systemapi::SystemAPI,
    token: String,
    node_info: NodeInfo,
) -> output::Object {
    let mut envelope = output::newfmt_envelope(system, token, &[]);
    let (mut data, mut attrs) = output::newfmt_data(system, DATA_TAG_SYSINFO);
    attrs.push_s(SYSINFO_ATTRIBUTES_NODE, node_info.node.clone());
    attrs.push_s(SYSINFO_ATTRIBUTES_OS_NAME, system.get_os_name());
    attrs.push_s(SYSINFO_ATTRIBUTES_OS_RELEASE, system.get_os_release());
    attrs.push_u(SYSINFO_ATTRIBUTES_SOCKETS, node_info.sockets);
    attrs.push_u(
        SYSINFO_ATTRIBUTES_CORES_PER_SOCKET,
        node_info.cores_per_socket,
    );
    attrs.push_u(
        SYSINFO_ATTRIBUTES_THREADS_PER_CORE,
        node_info.threads_per_core,
    );
    attrs.push_s(
        SYSINFO_ATTRIBUTES_CPU_MODEL,
        node_info.cores[0].model_name.clone(),
    );
    attrs.push_s(SYSINFO_ATTRIBUTES_ARCHITECTURE, system.get_architecture());
    attrs.push_u(SYSINFO_ATTRIBUTES_MEMORY, node_info.mem_kb);
    let mut distances = output::Array::new();
    for row in node_info.distances.iter() {
        let mut r = output::Array::new();
        for elt in row.iter() {
            r.push_u(*elt as u64);
        }
        distances.push(output::Value::A(r));
    }
    attrs.push_a(SYSINFO_ATTRIBUTES_DISTANCES, distances);
    if let Some(ref topo_svg) = node_info.topo_svg {
        attrs.push_s(
            SYSINFO_ATTRIBUTES_TOPO_SVG,
            STANDARD.encode(topo_svg.to_string()),
        );
    }
    if let Some(ref topo_text) = node_info.topo_text {
        attrs.push_s(
            SYSINFO_ATTRIBUTES_TOPO_TEXT,
            STANDARD.encode(topo_text.to_string()),
        );
    }
    let gpu_info = layout_card_info_newfmt(&node_info);
    if gpu_info.len() > 0 {
        attrs.push_a(SYSINFO_ATTRIBUTES_CARDS, gpu_info);
    }
    data.push_o(SYSINFO_DATA_ATTRIBUTES, attrs);
    envelope.push_o(SYSINFO_ENVELOPE_DATA, data);
    envelope
}

fn layout_error_newfmt(
    system: &dyn systemapi::SystemAPI,
    token: String,
    error: String,
) -> output::Object {
    let mut envelope = output::newfmt_envelope(system, token, &[]);
    envelope.push_a(
        SYSINFO_ENVELOPE_ERRORS,
        output::newfmt_one_error(system, error),
    );
    envelope
}

fn layout_card_info_newfmt(node_info: &NodeInfo) -> output::Array {
    let mut gpu_info = output::Array::new();
    for c in &node_info.cards {
        let gpu::Card {
            device,
            bus_addr,
            manufacturer,
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
        gpu.push_i(SYSINFO_GPU_CARD_INDEX, device.index as i64);
        gpu.push_s(SYSINFO_GPU_CARD_UUID, device.uuid.to_string());
        if bus_addr != "" {
            gpu.push_s(SYSINFO_GPU_CARD_ADDRESS, bus_addr.to_string());
        }
        if manufacturer != "" {
            gpu.push_s(SYSINFO_GPU_CARD_MANUFACTURER, manufacturer.clone());
        }
        if model != "" {
            gpu.push_s(SYSINFO_GPU_CARD_MODEL, model.to_string());
        }
        if arch != "" {
            gpu.push_s(SYSINFO_GPU_CARD_ARCHITECTURE, arch.to_string());
        }
        if driver != "" {
            gpu.push_s(SYSINFO_GPU_CARD_DRIVER, driver.to_string());
        }
        if firmware != "" {
            gpu.push_s(SYSINFO_GPU_CARD_FIRMWARE, firmware.to_string());
        }
        if *mem_size_kib != 0 {
            gpu.push_u(SYSINFO_GPU_CARD_MEMORY, *mem_size_kib);
        }
        if *power_limit_watt != 0 {
            gpu.push_i(SYSINFO_GPU_CARD_POWER_LIMIT, *power_limit_watt as i64);
        }
        if *max_power_limit_watt != 0 {
            gpu.push_i(
                SYSINFO_GPU_CARD_MAX_POWER_LIMIT,
                *max_power_limit_watt as i64,
            );
        }
        if *min_power_limit_watt != 0 {
            gpu.push_i(
                SYSINFO_GPU_CARD_MIN_POWER_LIMIT,
                *min_power_limit_watt as i64,
            );
        }
        if *max_ce_clock_mhz != 0 {
            gpu.push_i(SYSINFO_GPU_CARD_MAX_CECLOCK, *max_ce_clock_mhz as i64);
        }
        if *max_mem_clock_mhz != 0 {
            gpu.push_i(SYSINFO_GPU_CARD_MAX_MEMORY_CLOCK, *max_mem_clock_mhz as i64);
        }
        gpu_info.push_o(gpu);
    }
    gpu_info
}

// Old JSON/CSV format - this is flattish (to accomodate CSV) and has some idiosyncratic field names.

//+ignore-strings
fn layout_sysinfo_oldfmt(system: &dyn systemapi::SystemAPI, node_info: NodeInfo) -> output::Object {
    let gpu_info = layout_card_info_oldfmt(&node_info);
    let mut sysinfo = output::Object::new();
    sysinfo.push_s("version", system.get_version());
    sysinfo.push_s("timestamp", system.get_timestamp());
    sysinfo.push_s("hostname", system.get_hostname());
    sysinfo.push_s("description", node_info.description);
    sysinfo.push_u("cpu_cores", node_info.logical_cores);
    sysinfo.push_u("mem_gb", node_info.mem_kb / (1024 * 1024));
    if node_info.gpu_cards != 0 {
        sysinfo.push_u("gpu_cards", node_info.gpu_cards);
        if node_info.gpumem_kb != 0 {
            sysinfo.push_u("gpumem_gb", node_info.gpumem_kb / (1024 * 1024));
        }
        if gpu_info.len() > 0 {
            sysinfo.push_a("gpu_info", gpu_info);
        }
    }
    sysinfo
}

fn layout_error_oldfmt(system: &dyn systemapi::SystemAPI, error: String) -> output::Object {
    let mut sysinfo = output::Object::new();
    sysinfo.push_s("version", system.get_version());
    sysinfo.push_s("timestamp", system.get_timestamp());
    sysinfo.push_s("hostname", system.get_hostname());
    sysinfo.push_s("error", error);
    sysinfo
}

fn layout_card_info_oldfmt(node_info: &NodeInfo) -> output::Array {
    let mut gpu_info = output::Array::new();
    for c in &node_info.cards {
        let gpu::Card {
            device,
            bus_addr,
            manufacturer,
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
        gpu.push_s("manufacturer", manufacturer.to_string());
        gpu.push_s("model", model.to_string());
        gpu.push_s("arch", arch.to_string());
        gpu.push_s("driver", driver.to_string());
        gpu.push_s("firmware", firmware.to_string());
        gpu.push_u("mem_size_kib", *mem_size_kib);
        gpu.push_i("power_limit_watt", *power_limit_watt as i64);
        gpu.push_i("max_power_limit_watt", *max_power_limit_watt as i64);
        gpu.push_i("min_power_limit_watt", *min_power_limit_watt as i64);
        gpu.push_i("max_ce_clock_mhz", *max_ce_clock_mhz as i64);
        gpu.push_i("max_mem_clock_mhz", *max_mem_clock_mhz as i64);
        gpu_info.push_o(gpu);
    }
    gpu_info
}
//-ignore-strings

const GIB: usize = 1024 * 1024 * 1024;

struct NodeInfo {
    node: String,
    description: String,
    logical_cores: u64,
    sockets: u64,
    cores_per_socket: u64,
    threads_per_core: u64,
    cores: Vec<systemapi::CoreInfo>,
    mem_kb: u64,
    gpu_cards: u64,
    gpumem_kb: u64,
    cards: Vec<gpu::Card>,
    distances: Vec<Vec<u32>>, // square matrix
    topo_svg: Option<String>,
    topo_text: Option<String>,
}

fn compute_nodeinfo(system: &dyn systemapi::SystemAPI) -> Result<NodeInfo, String> {
    let gpus = system.get_gpus();
    let systemapi::CpuInfo {
        sockets,
        cores_per_socket,
        threads_per_core,
        cores,
    } = system.get_cpu_info()?;
    let model_name = cores[0].model_name.clone(); // expedient
    let memory = system.get_memory_in_kib()?;
    let mem_kb = memory.total;
    let mem_gb = (mem_kb as f64 / (1024.0 * 1024.0)).round() as u64;
    let mut cards = match gpus.probe() {
        Some(device) => device.get_card_configuration().unwrap_or_default(),
        None => vec![],
    };
    let ht = if threads_per_core > 1 {
        " (hyperthreaded)"
    } else {
        ""
    };
    let distances = system.get_numa_distances()?;

    let (gpu_desc, gpu_cards, gpumem_kb) = if !cards.is_empty() {
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
                ((cards[first].mem_size_kib as f64 * 1024.0 / GIB as f64).round() as usize)
                    .to_string()
            } else {
                "unknown ".to_string()
            };
            gpu_desc += &format!(", {}x {} @ {}GiB", (i - first), cards[first].model, memsize);
        }

        // Compute aggregate data
        let gpu_cards = cards.len() as i32;
        let mut total_mem_kb = 0u64;
        for c in &cards {
            total_mem_kb += c.mem_size_kib;
        }
        (gpu_desc, gpu_cards, total_mem_kb)
    } else {
        ("".to_string(), 0, 0)
    };

    Ok(NodeInfo {
        node: system.get_hostname(),
        description: format!(
            "{sockets}x{cores_per_socket}{ht} {model_name}, {mem_gb} GiB{gpu_desc}"
        ),
        logical_cores: (sockets * cores_per_socket * threads_per_core) as u64,
        sockets: sockets as u64,
        cores_per_socket: cores_per_socket as u64,
        threads_per_core: threads_per_core as u64,
        cores,
        mem_kb,
        gpu_cards: gpu_cards as u64,
        gpumem_kb,
        cards,
        distances,
        topo_svg: None,
        topo_text: None,
    })
}
