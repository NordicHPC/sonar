use crate::mocksystem;
use crate::ps;

#[test]
pub fn test_ps_no_meminfo() {
    let system = mocksystem::MockSystem::new().
        with_timestamp("2025-02-17T12:54:12+01:00").
        with_hostname("yes.no").
        with_version("1.2.3").
        freeze();

    let mut output = Vec::new();
    let options = ps::PsOptions {
        always_print_something: true,
        new_json: true,
        ..Default::default()
    };
    ps::create_snapshot(&mut output, &system, &options);
    let info = String::from_utf8_lossy(&output);
    let expect = r#"{"v":"1.2.3","time":"2025-02-17T12:54:12+01:00","host":"yes.no","user":"_sonar_","cmd":"_heartbeat_","error":"Unable to read /proc/meminfo"}
"#;
    assert!(info == expect);
}
