// These are separated so as not to confuse some test code that greps output.rs for strings that
// look like field names.

use crate::output::{write_json, Array, Object, Value};

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
