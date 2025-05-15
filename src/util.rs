#![allow(unused_imports)]
#![allow(unused_macros)]

use std::ffi::CStr;

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

// If the value contains a , or " then quote the string, and double every "
pub fn csv_quote(s: &str) -> String {
    let mut t = "".to_string();
    let mut must_quote = false;
    for c in s.chars() {
        match c {
            '"' => {
                t.push(c);
                t.push(c);
                must_quote = true;
            }
            ',' => {
                t.push(c);
                must_quote = true;
            }
            _ => {
                t.push(c);
            }
        }
    }
    if must_quote {
        t = "\"".to_string() + &t + "\""
    }
    t
}

#[test]
pub fn csv_quote_test() {
    assert!(&csv_quote("abcde") == "abcde");
    assert!(&csv_quote(r#"abc,de"#) == r#""abc,de""#);
    assert!(&csv_quote(r#"abc"de"#) == r#""abc""de""#);
    assert!(&csv_quote(r#"abc""de"#) == r#""abc""""de""#);
}

// Copy a C string.

pub fn cstrdup(s: &[cty::c_char]) -> String {
    unsafe { CStr::from_ptr(s.as_ptr()) }
        .to_str()
        .expect("Will always be utf8")
        .to_string()
}

// Generate low-quality but fast randomish u32 numbers.

#[allow(dead_code)]
pub struct Rng {
    state: u32, // nonzero
}

#[allow(dead_code)]
impl Rng {
    pub fn new() -> Rng {
        Rng {
            state: crate::time::unix_now() as u32,
        }
    }

    // https://en.wikipedia.org/wiki/Xorshift, this supposedly has period 2^32-1 but is not "very
    // random".
    pub fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[test]
pub fn rng_test() {
    let mut r = Rng::new();
    let a = r.next();
    let b = r.next();
    let c = r.next();
    let d = r.next();
    // It's completely unlikely that they're all equal, so that would indicate some kind of bug.
    assert!(!(a == b && b == c && c == d));
}
