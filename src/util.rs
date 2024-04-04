#![allow(unused_imports)]
#![allow(unused_macros)]

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
