use std::time::Duration;
use subprocess::{Exec, Redirection};

// the pipe is here as a workaround for https://github.com/rust-lang/rust/issues/45572
// see also https://doc.rust-lang.org/std/process/index.html
pub fn safe_command(command: &str, timeout_seconds: u64) -> Option<String> {
    let mut p = Exec::shell(command)
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Pipe)
        .popen()
        .ok()?;

    if (p.wait_timeout(Duration::new(timeout_seconds, 0)).ok()?).is_some() {
        let (out, err) = p.communicate(None).ok()?;

        match err {
            Some(e) => {
                if e.is_empty() {
                    out
                } else {
                    None
                }
            }
            None => out,
        }
    } else {
        p.kill().ok()?;
        p.wait().ok()?;

        None
    }
}
