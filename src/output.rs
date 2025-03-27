// Define a nested data structure of arrays, objects, and scalar values that can subsequently be
// serialized, currently as CSV and JSON, following conventions that are backward compatible with
// the older ad-hoc Sonar formatting code.
//
// Adding eg a compact binary serialization form would be very simple.

use crate::systemapi;
use crate::util;

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
    nonempty_base45: bool,
    sep: String,
}

#[allow(dead_code)]
impl Array {
    pub fn new() -> Array {
        Array {
            elements: vec![],
            nonempty_base45: false,
            sep: ",".to_string(),
        }
    }

    pub fn from_vec(elements: Vec<Value>) -> Array {
        Array {
            elements,
            nonempty_base45: false,
            sep: ",".to_string(),
        }
    }

    pub fn take(&mut self) -> Vec<Value> {
        let mut n = vec![];
        std::mem::swap(&mut n, &mut self.elements);
        n
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

    // This creates a constraint that:
    //
    // - there must be at least one element
    // - all elements must be Value::U
    // - the array is encoded as an offsetted little-endian base45 string (below).
    //
    // This is an efficient and CSV-friendly encoding of a typical array of cpu-second data.
    pub fn set_encode_nonempty_base45(&mut self) {
        self.nonempty_base45 = true;
    }

    // Use sep as a CSV array separator instead of the default ",".
    pub fn set_csv_separator(&mut self, sep: String) {
        self.sep = sep;
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
    if a.nonempty_base45 {
        let us = a
            .elements
            .iter()
            .map(|x| {
                if let Value::U(u) = x {
                    *u
                } else {
                    panic!("Not a Value::U")
                }
            })
            .collect::<Vec<u64>>();
        write_chars(writer, &encode_cpu_secs_base45el(&us));
        return;
    }

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

fn write_json_string(writer: &mut dyn io::Write, s: &String) {
    let _ = writer.write(b"\"");
    write_chars(writer, &util::json_quote(s));
    let _ = writer.write(b"\"");
}

// CSV:
//
// - an object is a comma-separated list of FIELDs
// - an array is an X-separated list of VALUEs (where X is comma by default but can be changed)
// - a TAG is an unquoted string
// - each FIELD is {TAG}={VALUE}
// - a VALUE is the string representation of the value
// - if the FIELD of an object or the VALUE of an array contains ',' or '"', then the FIELD or VALUE
//   is prefixed and suffixed by '"' and any '"' in the original string is doubled.
//
// Note that the bare representation of a value of any kind is just the string representation of the
// value itself (unquoted), it's the inclusion in an object or array that forces the quoting.
//
// The format allows nesting but the number of " grows exponentially with the nesting level if array
// separators are not managed carefully.  Also, custom array element separators are not handled
// specially by the quoting mechanism, effectively requiring each nesting level to have its own
// custom quoting mechanism and to avoid quoting chars used at outer levels.  For data nested more
// than one level, and especially when those data include arbitrary strings, use JSON.

pub fn write_csv(writer: &mut dyn io::Write, v: &Value) {
    write_chars(writer, &format_csv_value(v));
    let _ = writer.write(b"\n");
}

pub fn format_csv_value(v: &Value) -> String {
    match v {
        Value::A(a) => format_csv_array(a),
        Value::O(o) => format_csv_object(o),
        Value::S(s) => s.clone(),
        Value::U(u) => format!("{u}"),
        Value::I(i) => format!("{i}"),
        Value::F(f) => format!("{f}"),
        Value::B(b) => format!("{b}"),
        Value::E() => "".to_string(),
    }
}

fn format_csv_object(o: &Object) -> String {
    let mut first = true;
    let mut s = "".to_string();
    for fld in &o.fields {
        if !first {
            s += ","
        }
        let mut tmp = fld.tag.clone();
        tmp += "=";
        tmp += &format_csv_value(&fld.value);
        s += &util::csv_quote(&tmp);
        first = false;
    }
    s
}

fn format_csv_array(a: &Array) -> String {
    if a.nonempty_base45 {
        let us = a
            .elements
            .iter()
            .map(|x| {
                if let Value::U(u) = x {
                    *u
                } else {
                    panic!("Not a Value::U")
                }
            })
            .collect::<Vec<u64>>();
        return encode_cpu_secs_base45el(&us);
    }
    let mut first = true;
    let mut s = "".to_string();
    for elt in &a.elements {
        if !first {
            s += &a.sep;
        }
        s += &util::csv_quote(&format_csv_value(elt));
        first = false;
    }
    s
}

// Encode a nonempty u64 array compactly.
//
// The output must be ASCII text (32 <= c < 128), ideally without ',' or '"' or '\' or ' ' to not
// make it difficult for the various output formats we use.  Also avoid DEL, because it is a weird
// control character.
//
// We have many encodings to choose from, see https://github.com/NordicHPC/sonar/issues/178.
//
// The values to be represented are always cpu seconds of active time since boot, one item per cpu,
// and it is assumed that they are roughly in the vicinity of each other (the largest is rarely more
// than 4x the smallest, say).  The assumption does not affect correctness, only compactness.
//
// The encoding first finds the minimum input value and subtracts that from all entries.  The
// minimum value, and all the entries, are then emitted as unsigned little-endian base-45 with the
// initial digit chosen from a different character set to indicate that it is initial.

fn encode_cpu_secs_base45el(cpu_secs: &[u64]) -> String {
    let base = *cpu_secs
        .iter()
        .reduce(std::cmp::min)
        .expect("Must have a non-empty array");
    let mut s = encode_u64_base45el(base);
    for x in cpu_secs {
        s += encode_u64_base45el(*x - base).as_str();
    }
    s
}

// The only character unused by the encoding, other than the ones we're not allowed to use, is '='.
const BASE: u64 = 45;
const INITIAL: &[u8] = "(){}[]<>+-abcdefghijklmnopqrstuvwxyz!@#$%^&*_".as_bytes();
const SUBSEQUENT: &[u8] = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ~|';:.?/`".as_bytes();

fn encode_u64_base45el(mut x: u64) -> String {
    let mut s = String::from(INITIAL[(x % BASE) as usize] as char);
    x /= BASE;
    while x > 0 {
        s.push(SUBSEQUENT[(x % BASE) as usize] as char);
        x /= BASE;
    }
    s
}

#[test]
pub fn test_encoding() {
    assert!(INITIAL.len() == BASE as usize);
    assert!(SUBSEQUENT.len() == BASE as usize);
    // This should be *1, *0, *29, *43, 1, *11 with * denoting an INITIAL char.
    let v = vec![1, 30, 89, 12];
    println!("{}", encode_cpu_secs_base45el(&v));
    assert!(encode_cpu_secs_base45el(&v) == ")(t*1b");
}

// Utilities

pub struct AttrVal {
    pub key: String,
    pub value: String,
}

pub fn newfmt_envelope(
    system: &dyn systemapi::SystemAPI,
    attrs: &[AttrVal],
) -> Object {
    let mut envelope = Object::new();
    let mut meta = Object::new();
    meta.push_s("producer","sonar".to_string());
    meta.push_s("version", system.get_version());
    // meta.push_u("format", 1) // 1 is the default
    // meta.push_s("token") // FIXME
    if attrs.len() > 0 {
        let mut attrvals = Array::new();
        for AttrVal { key, value } in attrs {
            let mut pair = Object::new();
            pair.push_s("key", key.clone());
            pair.push_s("value", value.clone());
            attrvals.push_o(pair);
        }
        meta.push_a("attrs", attrvals);
    }
    envelope.push_o("meta", meta);
    envelope
}

pub fn newfmt_data(system: &dyn systemapi::SystemAPI, ty: &str) -> (Object, Object) {
    let mut data = Object::new();
    data.push_s("type",ty.to_string());
    let mut attrs = Object::new();
    attrs.push_s("time", system.get_timestamp());
    let c = system.get_cluster();
    if c != "" {
        attrs.push_s("cluster", c);
    }
    (data, attrs)
}

pub fn newfmt_one_error(system: &dyn systemapi::SystemAPI, error: String) -> Array {
    let mut err0 = Object::new();
    err0.push_s("detail", error);
    err0.push_s("time", system.get_timestamp());
    let mut errors = Array::new();
    errors.push_o(err0);
    errors
}
