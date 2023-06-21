use std::time::Duration;
use subprocess::{Exec, ExitStatus, Redirection};

#[derive(Debug, Clone, PartialEq)]
pub enum CmdError {
    CouldNotStart,
    Failed,
    Hung,
    InternalError,
}

// the pipe is here as a workaround for https://github.com/rust-lang/rust/issues/45572
// see also https://doc.rust-lang.org/std/process/index.html

pub fn safe_command(command: &str, timeout_seconds: u64) -> Result<String, CmdError> {
    let mut p = match Exec::shell(command)
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Pipe)
        .popen()
    {
        Ok(p) => p,
        Err(_) => {
            // TODO: Possibly too coarse-grained but the documentation is not
            // helpful in clarifying what might have happened.
            return Err(CmdError::CouldNotStart);
        }
    };

    match p.wait_timeout(Duration::new(timeout_seconds, 0)) {
        Ok(Some(ExitStatus::Exited(0))) => match p.communicate(None) {
            Ok((Some(stdout), Some(stderr))) => {
                if stderr.is_empty() {
                    Ok(stdout)
                } else {
                    Err(CmdError::Failed)
                }
            }
            Ok((_, _)) => Err(CmdError::InternalError),
            Err(_) => Err(CmdError::Failed),
        },
        Ok(Some(ExitStatus::Exited(126))) => {
            // 126 == "Command cannot execute"
            Err(CmdError::CouldNotStart)
        }
        Ok(Some(ExitStatus::Exited(127))) => {
            // 127 == "Command not found"
            Err(CmdError::CouldNotStart)
        }
        Ok(Some(_)) => Err(CmdError::Failed),
        Ok(None) => {
            // Timeout
            if p.kill().is_ok() && p.wait().is_ok() {
                Err(CmdError::Hung)
            } else {
                Err(CmdError::InternalError)
            }
        }
        Err(_) => Err(CmdError::InternalError),
    }
}

#[test]
fn test_safe_command() {
    // Should work, because we should be running this in the repo root.
    match safe_command("ls Cargo.toml", 2) {
	Ok(_) => {},
	Err(_) => assert!(false)
    }
    // This really needs to be the output
    assert!(safe_command("grep '^name =' Cargo.toml", 2) == Ok("name = \"sonar\"\n".to_string()));
    // Not found
    assert!(safe_command("no-such-command-we-hope", 2) == Err(CmdError::CouldNotStart));
    // Wrong permissions, not executable
    assert!(safe_command("/etc/passwd", 2) == Err(CmdError::CouldNotStart));
    // Should take too long
    assert!(safe_command("traceroute amazon.com", 2) == Err(CmdError::Hung));
    // Exited with error
    assert!(safe_command("ls /abracadabra", 2) == Err(CmdError::Failed));
}
