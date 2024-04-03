#![allow(unused_imports)]
#![allow(unused_macros)]

use chrono::prelude::Local;

// Populate a HashSet.
#[cfg(test)]
macro_rules! set(
    { $($key:expr),+ } => {
        {
            let mut m = ::std::collections::HashSet::new();
            $(
                m.insert($key);
            )+
            m
        }
     };
);

// Populate a HashMap.
#[cfg(test)]
macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

#[cfg(test)]
pub(crate) use map;

#[cfg(test)]
pub(crate) use set;

// Get current time as an ISO time stamp.
pub fn time_iso8601() -> String {
    let local_time = Local::now();
    format!("{}", local_time.format("%Y-%m-%dT%H:%M:%S%Z"))
}

// Carve up a line of text into space-separated chunks + the start indices of the chunks.
pub fn chunks(input: &str) -> (Vec<usize>, Vec<&str>) {
    let mut start_indices: Vec<usize> = Vec::new();
    let mut parts: Vec<&str> = Vec::new();

    let mut last_index = 0;
    for (index, c) in input.char_indices() {
        if c.is_whitespace() {
            if last_index != index {
                start_indices.push(last_index);
                parts.push(&input[last_index..index]);
            }
            last_index = index + 1;
        }
    }

    if last_index < input.len() {
        start_indices.push(last_index);
        parts.push(&input[last_index..]);
    }

    (start_indices, parts)
}

// Round `n` to 3 decimal places.
pub fn three_places(n: f64) -> f64 {
    (n * 1000.0).round() / 1000.0
}

// Insert \ before " and \
// Insert escape sequences for well-known control chars.
// Translate all other control chars to spaces (it's possible to do better).
pub fn json_quote(s: &str) -> String {
    let mut t = "".to_string();
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                t.push('\\');
                t.push(c);
            }
            '\n' => {
                t.push_str("\\n");
            }
            '\r' => {
                t.push_str("\\r");
            }
            '\t' => {
                t.push_str("\\t");
            }
            _ctl if c < ' ' => {
                t.push(' ');
            }
            _ => {
                t.push(c);
            }
        }
    }
    t
}

#[test]
pub fn json_quote_test() {
    assert!(&json_quote("abcde") == "abcde");
    assert!(&json_quote(r#"abc\de"#) == r#"abc\\de"#);
    assert!(&json_quote(r#"abc"de"#) == r#"abc\"de"#);
    assert!(&json_quote("abc\nde") == r#"abc\nde"#);
    assert!(&json_quote("abc\rde") == r#"abc\rde"#);
    assert!(&json_quote("abc	de") == r#"abc\tde"#);
    assert!(&json_quote("abc\u{0008}de") == r#"abc de"#);
}
