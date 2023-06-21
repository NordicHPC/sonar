use std::time::Duration;
use subprocess::{Exec, ExitStatus, Redirection};

#[derive(Debug, Clone)]
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
        Err(_) => return Err(CmdError::CouldNotStart),
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
