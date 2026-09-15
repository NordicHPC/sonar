use std::time::Duration;
use subprocess::{Exec, Redirection};

#[derive(Debug, PartialEq)]
pub enum CmdError {
    CouldNotStart(String),
    Failed(String),
    Hung(String),
    InternalError(String),
}

// There's a general problem with subprocesses writing to a pipe in that there is a limited capacity
// in the pipe (it can be on the language library side and/or on the OS side, it doesn't matter too
// much).  When the pipe fills up the child stops, which means that we'll time out if we use a
// timeout or will hang indefinitely if not; this is a problem for subprocesses that produce a lot
// of output, as they sometimes will (an unfiltered ps command on a large system produces thousands
// of lines).
//
// The solution for this problem is to be sure to drain the pipe while we are also waiting for the
// termination of the child.  This is explained at https://github.com/rust-lang/rust/issues/45572,
// especially at https://github.com/rust-lang/rust/issues/45572#issuecomment-860134955.  See also
// https://doc.rust-lang.org/std/process/index.html (second code blob under "Handling I/O").
//
// The process crate claims that it does all of this, we don't need to implement that ourselves.

pub fn safe_command(
    command: &str,
    args: &[&str],
    timeout_seconds: u64,
) -> Result<(String, String), CmdError> {
    let job = match Exec::cmd(command)
        .args(args)
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Pipe)
        .start()
    {
        Ok(job) => job,
        Err(_) => {
            return Err(CmdError::CouldNotStart(command.to_string()));
        }
    };
    let cap = match job.capture_timeout(Duration::new(timeout_seconds, 0)) {
        Ok(cap) => cap,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::TimedOut {
                return Err(CmdError::Hung(format_failure(
                    command,
                    "Timed out and had to be killed",
                    "",
                    "",
                )));
            }
            return Err(CmdError::InternalError(format_failure(
                command,
                format!("Unknown internal error {:?}", e).as_str(),
                "",
                "",
            )));
        }
    };
    let stdout_result = cap.stdout_str();
    let stderr_result = cap.stderr_str();
    match cap.exit_status.code() {
        Some(0) => Ok((stdout_result, stderr_result)),
        Some(126) => Err(CmdError::CouldNotStart(format_failure(
            command,
            "Command cannot execute",
            &stdout_result,
            &stderr_result,
        ))),
        Some(127) => Err(CmdError::CouldNotStart(format_failure(
            command,
            "Command not found",
            &stdout_result,
            &stderr_result,
        ))),
        Some(x) => Err(CmdError::Failed(format_failure(
            command,
            format!("Unspecified other exit status {:?}", x).as_str(),
            &stdout_result,
            &stderr_result,
        ))),
        None => match cap.exit_status.signal() {
            Some(15) => Err(CmdError::Hung(format_failure(
                command,
                "Killed by SIGTERM",
                &stdout_result,
                &stderr_result,
            ))),
            Some(_) => Err(CmdError::Hung(format_failure(
                command,
                "Killed by signal",
                &stdout_result,
                &stderr_result,
            ))),
            None => Err(CmdError::InternalError(format_failure(
                command,
                "Internal error",
                &stdout_result,
                &stderr_result,
            ))),
        },
    }
}

fn format_failure(command: &str, root_cause: &str, stdout: &str, stderr: &str) -> String {
    if !stdout.is_empty() {
        if !stderr.is_empty() {
            format!(
                "COMMAND:\n{command}\nROOT CAUSE:\n{root_cause}\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
            )
        } else {
            format!("COMMAND:\n{command}\nROOT CAUSE:\n{root_cause}\nSTDOUT:\n{stdout}")
        }
    } else if !stderr.is_empty() {
        format!("COMMAND:\n{command}\nROOT CAUSE:\n{root_cause}\nSTDERR:\n{stderr}")
    } else {
        format!("COMMAND:\n{command}\nROOT CAUSE:\n{root_cause}")
    }
}

#[test]
fn test_safe_command() {
    // Should work, because we should be running this in the repo root.
    match safe_command("ls", &["Cargo.toml"], 2) {
        Ok(_) => {}
        Err(_) => assert!(false),
    }
    // This really needs to be the output
    match safe_command("grep", &["^name =", "Cargo.toml"], 2) {
        Ok((s, _)) => {
            assert!(s == "name = \"sonar\"\n".to_string())
        }
        Err(_) => {
            assert!(false)
        }
    }
    // Not found
    match safe_command("no-such-command-we-hope", &[], 2) {
        Err(CmdError::CouldNotStart(_)) => {}
        _ => {
            assert!(false)
        }
    }
    // Wrong permissions, not executable
    match safe_command("/etc/passwd", &[], 2) {
        Err(CmdError::CouldNotStart(_)) => {}
        _ => {
            assert!(false)
        }
    }
    // Should take too long
    match safe_command("sleep", &["7"], 2) {
        Err(CmdError::Hung(_)) => {}
        _ => {
            assert!(false)
        }
    }
    // Exited with error
    match safe_command("ls", &["/abracadabra"], 2) {
        Err(CmdError::Failed(_)) => {}
        _ => {
            assert!(false)
        }
    }
    // Should work even though output is large, but we want a file that is always present.  The file
    // img/sonar.png is 1.2MB, which is probably good enough.
    match safe_command("cat", &["img/sonar.png"], 5) {
        Ok(_) => {}
        Err(_) => {
            assert!(false)
        }
    }
    // Should return Ok but should produce output on both stdout and stderr.
    match safe_command("src/testdata/command-output.sh", &[], 5) {
        Ok((a, b)) => {
            assert!(a == "output from stdout\n".to_string());
            assert!(b == "output from stderr\n".to_string());
        }
        Err(_e) => {
            assert!(false)
        }
    }
}
