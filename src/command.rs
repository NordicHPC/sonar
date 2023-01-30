use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub fn safe_command(command: &str, args: Vec<&str>, timeout_seconds: u64) -> Option<String> {
    let mut child = Command::new(command)
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let duration = Duration::from_secs(timeout_seconds);
    match child.wait_timeout(duration).unwrap() {
        Some(_status) => {
            let out = child.wait_with_output().ok()?;
            Some(String::from_utf8(out.stdout).unwrap())
        }
        None => {
            // child hasn't exited yet
            child.kill().unwrap();
            child.wait().unwrap();
            None
        }
    }
}
