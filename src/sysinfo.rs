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
            Format::JSON,
            self.topo_svg_cmd.clone(),
            self.topo_text_cmd.clone(),
        );
        vec![writer]
    }
}

#[derive(Clone)]
pub enum Format {
    // There used to be CSV and OldJSON here, we might add eg Protobuf
    JSON,
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
                Format::JSON => layout_sysinfo_newfmt(system, token, info),
            }
        }
        Err(e) => match fmt {
            Format::JSON => layout_error_newfmt(system, token, e),
        },
    };
    match fmt {
        Format::JSON => {
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
    attrs.push_u(SYSINFO_ATTRIBUTES_NUMA_NODES, node_info.numa_nodes);
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
        attrs.push_s(SYSINFO_ATTRIBUTES_TOPO_SVG, STANDARD.encode(topo_svg));
    }
    if let Some(ref topo_text) = node_info.topo_text {
        attrs.push_s(SYSINFO_ATTRIBUTES_TOPO_TEXT, STANDARD.encode(topo_text));
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

struct NodeInfo {
    node: String,
    numa_nodes: u64,
    sockets: u64,
    cores_per_socket: u64,
    threads_per_core: u64,
    cores: Vec<systemapi::CoreInfo>,
    mem_kb: u64,
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
    let memory = system.get_memory_in_kib()?;
    let mem_kb = memory.total;
    let cards = match gpus.probe() {
        Some(device) => device.get_card_configuration().unwrap_or_default(),
        None => vec![],
    };
    let distances = system.get_numa_distances()?;
    Ok(NodeInfo {
        node: system.get_hostname(),
        numa_nodes: distances.len() as u64,
        sockets: sockets as u64,
        cores_per_socket: cores_per_socket as u64,
        threads_per_core: threads_per_core as u64,
        cores,
        mem_kb,
        cards,
        distances,
        topo_svg: None,
        topo_text: None,
    })
}
