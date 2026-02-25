#![allow(clippy::len_zero)]
#![allow(clippy::comparison_to_empty)]

// Define a nested data structure of arrays, objects, and scalar values that can subsequently be
// serialized, currently only as JSON.  Adding eg a compact binary serialization form would be very
// simple.

use crate::json_tags::*;
use crate::systemapi;
use crate::util::format;

use std::io;

#[derive(Debug)]
pub enum Value {
    A(Array),
    O(Object),
    S(String),
    U(u64),
    I(i64),
    F(f64),
    B(bool),
    E(), // Empty array element only, never a field or toplevel value
}

#[derive(Debug)]
struct Field {
    tag: String,
    value: Value,
}

#[derive(Debug)]
pub struct Object {
    fields: Vec<Field>,
}

#[allow(dead_code)]
impl Object {
    pub fn new() -> Object {
        Object { fields: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&Value> {
        for f in &self.fields {
            if key == &f.tag {
                return Some(&f.value);
            }
        }
        None
    }

    pub fn push(&mut self, tag: &str, value: Value) {
        self.fields.push(Field {
            tag: tag.to_string(),
            value,
        })
    }

    pub fn prepend(&mut self, tag: &str, value: Value) {
        self.fields.insert(
            0,
            Field {
                tag: tag.to_string(),
                value,
            },
        )
    }

    pub fn push_o(&mut self, tag: &str, o: Object) {
        self.push(tag, Value::O(o));
    }

    pub fn push_a(&mut self, tag: &str, a: Array) {
        self.push(tag, Value::A(a));
    }

    pub fn push_s(&mut self, tag: &str, s: String) {
        self.push(tag, Value::S(s));
    }

    pub fn prepend_s(&mut self, tag: &str, s: String) {
        self.prepend(tag, Value::S(s));
    }

    pub fn push_u(&mut self, tag: &str, u: u64) {
        self.push(tag, Value::U(u));
    }

    pub fn push_i(&mut self, tag: &str, i: i64) {
        self.push(tag, Value::I(i));
    }

    pub fn push_f(&mut self, tag: &str, f: f64) {
        self.push(tag, Value::F(f));
    }

    pub fn push_b(&mut self, tag: &str, b: bool) {
        self.push(tag, Value::B(b));
    }
}

#[derive(Debug)]
pub struct Array {
    elements: Vec<Value>,
}

#[allow(dead_code)]
impl Array {
    pub fn new() -> Array {
        Array { elements: vec![] }
    }

    pub fn from_vec(elements: Vec<Value>) -> Array {
        Array { elements }
    }

    pub fn take(&mut self) -> Vec<Value> {
        let mut n = vec![];
        std::mem::swap(&mut n, &mut self.elements);
        n
    }

    pub fn is_empty(&self) -> bool {
        self.elements.len() == 0
    }

    pub fn push(&mut self, value: Value) {
        self.elements.push(value)
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn at(&self, i: usize) -> &Value {
        &self.elements[i]
    }

    pub fn push_o(&mut self, o: Object) {
        self.push(Value::O(o));
    }

    pub fn push_s(&mut self, s: String) {
        self.push(Value::S(s));
    }

    pub fn push_u(&mut self, u: u64) {
        self.push(Value::U(u));
    }

    pub fn push_i(&mut self, i: i64) {
        self.push(Value::I(i));
    }

    pub fn push_f(&mut self, f: f64) {
        self.push(Value::F(f));
    }

    pub fn push_e(&mut self) {
        self.push(Value::E());
    }
}

// Write some data and ignore errors.

fn write_chars(writer: &mut dyn io::Write, s: &str) {
    let _ = writer.write(s.as_bytes());
}

// JSON output follows the standard.

pub fn write_json(writer: &mut dyn io::Write, v: &Value) {
    write_json_int(writer, v);
    let _ = writer.write(b"\n");
}

fn write_json_int(writer: &mut dyn io::Write, v: &Value) {
    match v {
        Value::A(a) => write_json_array(writer, a),
        Value::O(o) => write_json_object(writer, o),
        Value::S(s) => write_json_string(writer, s),
        Value::U(u) => write_chars(writer, &format!("{u}")),
        Value::I(i) => write_chars(writer, &format!("{i}")),
        Value::F(f) => write_chars(writer, &format!("{f}")),
        Value::B(b) => write_chars(writer, &format!("{b}")),
        Value::E() => {}
    }
}

fn write_json_array(writer: &mut dyn io::Write, a: &Array) {
    let _ = writer.write(b"[");
    let mut first = true;
    for elt in &a.elements {
        if !first {
            let _ = writer.write(b",");
        }
        write_json_int(writer, elt);
        first = false;
    }
    let _ = writer.write(b"]");
}

fn write_json_object(writer: &mut dyn io::Write, o: &Object) {
    let _ = writer.write(b"{");
    let mut first = true;
    for fld in &o.fields {
        if !first {
            let _ = writer.write(b",");
        }
        write_json_string(writer, &fld.tag);
        let _ = writer.write(b":");
        write_json_int(writer, &fld.value);
        first = false;
    }
    let _ = writer.write(b"}");
}

fn write_json_string(writer: &mut dyn io::Write, s: &str) {
    let _ = writer.write(b"\"");
    write_chars(writer, &format::json_quote(s));
    let _ = writer.write(b"\"");
}

// Utilities

pub struct AttrVal {
    pub key: String,
    pub value: String,
}

pub fn newfmt_envelope(
    system: &dyn systemapi::SystemAPI,
    token: String,
    attrs: &[AttrVal],
) -> Object {
    let mut envelope = Object::new();
    let mut meta = Object::new();
    let sonar = "sonar".to_string();
    meta.push_s(METADATA_OBJECT_PRODUCER, sonar);
    meta.push_s(METADATA_OBJECT_VERSION, system.get_version());
    if crate::OUTPUT_FORMAT != 0 {
        meta.push_u(METADATA_OBJECT_FORMAT, crate::OUTPUT_FORMAT)
    }
    if token != "" {
        meta.push_s(METADATA_OBJECT_TOKEN, token)
    }
    if attrs.len() > 0 {
        let mut attrvals = Array::new();
        for AttrVal { key, value } in attrs {
            let mut pair = Object::new();
            pair.push_s(KVPAIR_KEY, key.clone());
            pair.push_s(KVPAIR_VALUE, value.clone());
            attrvals.push_o(pair);
        }
        meta.push_a(METADATA_OBJECT_ATTRS, attrvals);
    }
    // NOTE - tag not specific to sysinfo
    envelope.push_o(SYSINFO_ENVELOPE_META, meta);
    assert!(CLUSTER_ENVELOPE_META == SYSINFO_ENVELOPE_META);
    assert!(SAMPLE_ENVELOPE_META == SYSINFO_ENVELOPE_META);
    assert!(JOBS_ENVELOPE_META == SYSINFO_ENVELOPE_META);
    envelope
}

pub fn newfmt_data(system: &dyn systemapi::SystemAPI, ty: &str) -> (Object, Object) {
    let mut data = Object::new();
    data.push_s(SYSINFO_DATA_TYPE, ty.to_string());
    // NOTE - tag not specific to sysinfo
    assert!(CLUSTER_DATA_TYPE == SYSINFO_DATA_TYPE);
    assert!(SAMPLE_DATA_TYPE == SYSINFO_DATA_TYPE);
    assert!(JOBS_DATA_TYPE == SYSINFO_DATA_TYPE);
    let mut attrs = Object::new();
    attrs.push_s(SYSINFO_ATTRIBUTES_TIME, system.get_timestamp());
    // NOTE - tag not specific to sysinfo
    assert!(CLUSTER_ATTRIBUTES_TIME == SYSINFO_ATTRIBUTES_TIME);
    assert!(SAMPLE_ATTRIBUTES_TIME == SYSINFO_ATTRIBUTES_TIME);
    assert!(JOBS_ATTRIBUTES_TIME == SYSINFO_ATTRIBUTES_TIME);
    let c = system.get_cluster();
    if c != "" {
        attrs.push_s(SYSINFO_ATTRIBUTES_CLUSTER, c);
        // NOTE - tag not specific to sysinfo
        assert!(CLUSTER_ATTRIBUTES_CLUSTER == SYSINFO_ATTRIBUTES_CLUSTER);
        assert!(SAMPLE_ATTRIBUTES_CLUSTER == SYSINFO_ATTRIBUTES_CLUSTER);
        assert!(JOBS_ATTRIBUTES_CLUSTER == SYSINFO_ATTRIBUTES_CLUSTER);
    }
    (data, attrs)
}

pub fn newfmt_one_error(system: &dyn systemapi::SystemAPI, error: String) -> Array {
    let mut err0 = Object::new();
    err0.push_s(ERROR_OBJECT_DETAIL, error);
    err0.push_s(ERROR_OBJECT_TIME, system.get_timestamp());
    let cluster = system.get_cluster();
    if cluster != "" {
        err0.push_s(ERROR_OBJECT_CLUSTER, cluster);
    }
    let node = system.get_hostname();
    if node != "" {
        err0.push_s(ERROR_OBJECT_NODE, node);
    }
    let mut errors = Array::new();
    errors.push_o(err0);
    errors
}
