use crate::linux::mocksystem;
use crate::ps;

#[test]
pub fn test_ps_no_meminfo() {
    let system = mocksystem::Builder::new()
        .with_timestamp("2025-02-17T12:54:12+01:00")
        .with_cluster("cl.no")
        .with_hostname("yes.no")
        .with_version("1.2.3")
        .freeze();

    let mut output = Vec::new();
    let options = ps::PsOptions {
        fmt: ps::Format::NewJSON,
        ..Default::default()
    };
    ps::create_snapshot(&mut output, &system, &options);
    let info = String::from_utf8_lossy(&output);
    let expect = r#"
{
"meta":{"producer":"sonar","version":"1.2.3"},
"errors":[{
"detail":"Unable to read /proc/stat",
"time":"2025-02-17T12:54:12+01:00",
"cluster":"cl.no",
"node":"yes.no"}]
}
"#;
    // println!("{}", info.replace('\n',""));
    // println!("{}", expect.replace('\n',""));
    assert!(info.replace('\n', "") == expect.replace('\n', ""));
}
