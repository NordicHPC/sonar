// Basically the `cluster` command is a wrapper around `sinfo`:
//
//  - Run sinfo to list partitions.
//  - Run sinfo to get a list of nodes broken down by partition and their state.
//
// The same could have been had in a different form by:
//
//  scontrol -o show nodes
//  scontrol -o show partitions
//
// Anyway, we emit a list of partitions with their nodes and a list of nodes with their states.
//
// If there is no sinfo, this emits an error.

use crate::output;
use crate::systemapi;
use crate::nodelist;

use std::io;

pub fn show_cluster(writer: &mut dyn io::Write, system: &dyn systemapi::SystemAPI) {
    output::write_json(
        writer,
        &output::Value::O(
            match do_show_cluster(system) {
                Ok(envelope) => envelope,
                Err(error) => {
                    let mut envelope = output::newfmt_envelope(system, &vec![]);
                    envelope.push_a("errors", output::newfmt_one_error(system, error));
                    envelope
                }
            }
        )
    )
}

fn do_show_cluster(system: &dyn systemapi::SystemAPI) -> Result<output::Object, String>{
    let mut partitions = output::Array::new();
    for (name, nodelist) in system.run_sinfo_partitions()? {
        let mut p = output::Object::new();
        // The default partition is marked but of no interest to us.
        let name = if let Some(suffix) = name.strip_suffix('*') {
            suffix.to_string()
	} else {
            name.to_string()
	};
        p.push_s("name", name);
        p.push_a("nodes", nodelist::parse_and_render(&nodelist)?);
        partitions.push_o(p);
    }

    let mut nodes = output::Array::new();
    for (nodelist, statelist) in system.run_sinfo_nodes()? {
        let mut p = output::Object::new();
        p.push_a("names", nodelist::parse_and_render(&nodelist)?);
        let mut states = output::Array::new();
        for s in statelist.split('+') {
            states.push_s(s.to_ascii_uppercase());
        }
        p.push_a("states", states);
        nodes.push_o(p);
    }

    let mut envelope = output::newfmt_envelope(system, &vec![]);
    let (mut data, mut attrs) = output::newfmt_data(system, "cluster");
    attrs.push_b("slurm", true);
    attrs.push_a("partitions", partitions);
    attrs.push_a("nodes", nodes);
    data.push_o("attributes", attrs);
    envelope.push_o("data", data);
    Ok(envelope)
}
