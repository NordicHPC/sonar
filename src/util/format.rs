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
