// On a batch system, obtain partitions+nodes and nodes+state and format them.  On non-batch
// systems, this emits an error.

use crate::json_tags::*;
use crate::nodelist;
use crate::output;
use crate::systemapi;

use std::io;

#[cfg(feature = "daemon")]
pub struct State<'a> {
    system: &'a dyn systemapi::SystemAPI,
    token: String,
}

#[cfg(feature = "daemon")]
impl<'a> State<'a> {
    pub fn new(system: &'a dyn systemapi::SystemAPI, token: String) -> State<'a> {
        State { system, token }
    }

    pub fn run(&mut self) -> Vec<Vec<u8>> {
        let mut writer = Vec::new();
        show_cluster(&mut writer, self.system, self.token.clone());
        vec![writer]
    }
}

pub fn show_cluster(writer: &mut dyn io::Write, system: &dyn systemapi::SystemAPI, token: String) {
    output::write_json(
        writer,
        &output::Value::O(match do_show_cluster(system, token.clone()) {
            Ok(envelope) => envelope,
            Err(error) => {
                let mut envelope = output::newfmt_envelope(system, token, &[]);
                envelope.push_a(
                    CLUSTER_ENVELOPE_ERRORS,
                    output::newfmt_one_error(system, error),
                );
                envelope
            }
        }),
    )
}

fn do_show_cluster(
    system: &dyn systemapi::SystemAPI,
    token: String,
) -> Result<output::Object, String> {
    let mut partitions = output::Array::new();
    for (name, nodelist) in system.compute_cluster_partitions()? {
        let mut p = output::Object::new();
        // The default partition is marked but of no interest to us.
        let name = if let Some(suffix) = name.strip_suffix('*') {
            suffix.to_string()
        } else {
            name.to_string()
        };
        p.push_s(CLUSTER_PARTITION_NAME, name);
        p.push_a(
            CLUSTER_PARTITION_NODES,
            nodelist::parse_and_render(&nodelist)?,
        );
        partitions.push_o(p);
    }

    let mut nodes = output::Array::new();
    for (nodelist, statelist) in system.compute_cluster_nodes()? {
        let mut p = output::Object::new();
        p.push_a(CLUSTER_NODES_NAMES, nodelist::parse_and_render(&nodelist)?);
        let mut states = output::Array::new();
        for s in statelist.split('+') {
            states.push_s(s.to_ascii_uppercase());
        }
        p.push_a(CLUSTER_NODES_STATES, states);
        nodes.push_o(p);
    }

    let mut envelope = output::newfmt_envelope(system, token, &[]);
    let (mut data, mut attrs) = output::newfmt_data(system, DATA_TAG_CLUSTER);
    match system.compute_cluster_kind() {
        Some(systemapi::ClusterKind::Slurm) => {
            attrs.push_b(CLUSTER_ATTRIBUTES_SLURM, true);
        }
        None => {
            return Err("Unknown batch system".to_string());
        }
    }
    attrs.push_a(CLUSTER_ATTRIBUTES_PARTITIONS, partitions);
    attrs.push_a(CLUSTER_ATTRIBUTES_NODES, nodes);
    data.push_o(CLUSTER_DATA_ATTRIBUTES, attrs);
    envelope.push_o(CLUSTER_ENVELOPE_DATA, data);
    Ok(envelope)
}
