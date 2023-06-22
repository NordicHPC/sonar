use std::io;
use std::time::Duration;
use subprocess::{Exec, ExitStatus, Redirection};

#[derive(Debug, Clone, PartialEq)]
pub enum CmdError {
    CouldNotStart,
    Failed,
    Hung,
    InternalError,
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

    // It's not necessary to use a thread here, we just limit the amount of time we're willing to
    // wait for output to become available.
    //
    // TODO: If the program produces one byte of output every timeout_seconds/2 seconds, say, then
    // we'll keep reading for as long as it does that, we won't abort the program after
    // timeout_seconds have passed.  I think this is probably OK even though it violates the letter
    // of the API.
    let mut comm = p
        .communicate_start(None)
        .limit_time(Duration::new(timeout_seconds, 0));
    let mut result = "".to_string();
    let code = loop {
        match comm.read_string() {
            Ok((Some(stdout), Some(stderr))) => {
                if !stderr.is_empty() {
                    break Some(CmdError::Failed);
                } else if stdout.is_empty() {
                    // This is always EOF because timeouts are signaled as Err()
                    break None;
                } else {
                    result = result + &stdout
                }
            }
            Ok((_, _)) => break Some(CmdError::InternalError),
            Err(e) => {
                if e.error.kind() == io::ErrorKind::TimedOut {
                    match p.terminate() {
                        Ok(_) => break Some(CmdError::Hung),
                        Err(_) => break Some(CmdError::InternalError),
                    }
                }
                break Some(CmdError::InternalError);
            }
        }
    };

    match p.wait() {
        Ok(ExitStatus::Exited(0)) => {
            if let Some(status) = code {
                Err(status)
            } else {
                Ok(result)
            }
        }
        Ok(ExitStatus::Exited(126)) => {
            // 126 == "Command cannot execute"
            Err(CmdError::CouldNotStart)
        }
        Ok(ExitStatus::Exited(127)) => {
            // 127 == "Command not found"
            Err(CmdError::CouldNotStart)
        }
        Ok(ExitStatus::Signaled(15)) => {
            // Signal 15 == SIGTERM
            Err(CmdError::Hung)
        }
        Ok(_) => Err(CmdError::Failed),
        Err(_) => Err(CmdError::InternalError),
    }
}

#[test]
fn test_safe_command() {
    // Should work, because we should be running this in the repo root.
    match safe_command("ls Cargo.toml", 2) {
        Ok(_) => {}
        Err(_) => assert!(false),
    }
    // This really needs to be the output
    assert!(safe_command("grep '^name =' Cargo.toml", 2) == Ok("name = \"sonar\"\n".to_string()));
    // Not found
    assert!(safe_command("no-such-command-we-hope", 2) == Err(CmdError::CouldNotStart));
    // Wrong permissions, not executable
    assert!(safe_command("/etc/passwd", 2) == Err(CmdError::CouldNotStart));
    // Should take too long
    assert!(safe_command("sleep 7", 2) == Err(CmdError::Hung));
    // Exited with error
    assert!(safe_command("ls /abracadabra", 2) == Err(CmdError::Failed));
    // Should work even though output is large (the executable is 26MB on my system)
    match safe_command("cat target/debug/sonar", 5) {
        Ok(_) => {}
        Err(_) => assert!(false),
    }
}
