// These are separated so as not to confuse some test code that greps output.rs for strings that
// look like field names.

use crate::output::{Array,Object,Value,write_json,write_csv};

#[test]
pub fn test_json() {
    let mut a = Array::new();
    let mut o = Object::new();
    o.push_o("o", Object::new());
    o.push_a("a", Array::new());
    o.push_s("s", r#"hello, "sir""#.to_string());
    o.push_u("u", 123);
    o.push_i("i", -12);
    o.push_f("f", 12.5);
    a.push_o(o);
    a.push_e();
    a.push_s(r#"stri\ng"#.to_string());
    let expect = concat!(
        r#"[{"o":{},"a":[],"s":"hello, \"sir\"","u":123,"i":-12,"f":12.5},,"stri\\ng"]"#,
        "\n",
    );
    let mut output = Vec::new();
    write_json(&mut output, &Value::A(a));
    let got = String::from_utf8_lossy(&output);
    assert!(expect == got);
}

#[test]
pub fn test_csv() {
    // The common (really only truly supported) case for CSV is that there's an object outermost.
    let mut o = Object::new();
    o.push_o("o", Object::new());
    let mut aa = Array::new();
    aa.push_i(1);
    aa.push_e();
    aa.push_i(2);
    aa.set_csv_separator("|".to_string());
    o.push_a("a", aa);
    o.push_s("s", r#"hello, "sir""#.to_string());
    o.push_u("u", 123);
    o.push_i("i", -12);
    o.push_f("f", 12.5);
    let mut ab = Array::new();
    ab.set_encode_nonempty_base45();
    // See the encoding test further down for an explanation of the encoded value.
    for x in vec![1, 30, 89, 12] {
        ab.push_u(x);
    }
    o.push_a("x", ab);
    let expect = concat!(
        r#"o=,a=1||2,"s=hello, ""sir""",u=123,i=-12,f=12.5,x=)(t*1b"#,
        "\n"
    );
    let mut output = Vec::new();
    write_csv(&mut output, &Value::O(o));
    let got = String::from_utf8_lossy(&output);
    assert!(expect == got);
}

