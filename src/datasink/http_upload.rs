use std::cmp::min;
use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel;

// The abstraction here will be a "http poster" that encapsulates the sending method and a bunch of
// the other stuff, and into which we can pump a bunch of data.  Then it does not matter if that
// uses curl or some other method.  The http poster can be shared between the kafka code and the
// pure http POST code, and has the same general shape as the kafka poster code.

struct HttpUploader {
    curl_cmd: &str,
    api_endpoint: &str,
    http_proxy: &str,
    timeout: u64,
}

impl HttpUploader {
    fn start(&self) -> Result<HttpUploadStream, String> {
        // Curl will retry for 1s, 2s, 4s, ..., 10m and then stick to 10m
        let mut retry_count = 0;
        let mut next = 1;
        while timeout > 0 {
            timeout -= min(next, timeout);
            next = min(600, next * 2);
            retry_count += 1;
        }
        let mut args = vec![
            "--silent".to_string(),
            "--show-error".to_string(),
            "--data-binary".to_string(),
            "@-".to_string(),
            "-H".to_string(),
            "Content-Type: application/octet-stream".to_string(),
        ];
        if retry_count > 0 {
            args.push("--retry".to_string());
            args.push(format!("{}", retry_count));
            args.push("--retry-connrefused".to_string());
        }
        args.push(api_endpoint.to_string());

        // Really want to merge stdout and stderr
        let mut cmd = std::process::Command::new(cmd_name);
        log::debug!("Curl: {cmd_name} {:?}", args);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if http_proxy != "" {
            cmd.env("http_proxy", http_proxy)
                .env("https_proxy", http_proxy);
        }
        match cmd.spawn() {
            Ok(mut child) => {
                if let (Some(mut stdin), Some(mut stdout), Some(mut stderr)) =
                    (child.stdin.take(), child.stdout.take(), child.stderr.take())
                {
                    Ok(HttpUploadStream{child, stdin, stdout, stderr, ...})
                } else {
                    // Should never happen, probably
                    Err("Failed to get stdin/stdout/stderr".to_string())
                }
            }
            Err(e) => {
                Err(format!("Failed to spawn curl: {:?}", e))
            }
        }
    }
}

struct HttpUploadStream {
    // child, stdin, stdout, stderr
}

impl HttpUploadStream {
    fn new(...) {
    }

    fn put(...) {
    }

    fn end(...) {
    }
}

// This needs to be figured out somehow but it's a complex of threads that handles i/o and waits on
// the child.

            drop(std::thread::spawn(move || {
                // get byte blobs from stream and write them to stdin
                for ... {
                    if let Err(err) = stdin.write_all(ctrl.as_bytes()) {
                        log::debug!("Failed to write control object: {:?}", err);
                    }
                    if let Err(err) = stdin.write_all(value.as_bytes()) {
                        log::debug!("Failed to write data blob size {data_size}: {:?}", err);
                    }
                }
                // Does this happen too soon?  As in, if there's enough data, will the consumer
                // not have finished consuming?  Or will it just flush things and all will be fine?
                drop(stdin);
            }));
                // Separate threads do the output consuming and waiting for curl to finish, in order to
                // guarantee several things:
                //
                //  - if the curl does not terminate immediately (because it is retrying, not
                //    reaching the host, bandwidth-limited, ...) then Sonar does not hang waiting
                //    for it, but can get on with its work.
                //  - curl will not block writing its output on a full pipe
                //  - sonar will not block on writing to curl because curl is blocked
                //  - the child does not linger once it's ready to exit
                //
                // We can't use wait_with_output() because that will close stdin, and we can't wait
                // to fork off these thread until writing has completed.  We could maybe combine the
                // two reader threads into one using some kind of nonblocking I/O.  Maybe there are
                // other tricks.
                //
                // 1K buffer is enough to see interesting errors.
                drop(std::thread::spawn(move || {
                    let mut buf = [0; 1024];
                    loop {
                        match stdout.read(&mut buf[..]) {
                            Err(_) | Ok(0) => {
                                break;
                            }
                            Ok(n) => {
                                // This is the common case even when curl fails to deliver when
                                // the server rejects the message.
                                log::debug!(
                                    "Curl succeeded with output: {}",
                                    String::from_utf8_lossy(&buf[..n])
                                );
                            }
                        }
                    }
                    drop(stdout);
                }));
                drop(std::thread::spawn(move || {
                    let mut buf = [0; 1024];
                    loop {
                        match stderr.read(&mut buf[..]) {
                            Err(_) | Ok(0) => {
                                break;
                            }
                            Ok(n) => {
                                log::debug!(
                                    "Curl failed with output {}",
                                    String::from_utf8_lossy(&buf[..n])
                                );
                            }
                        }
                    }
                    drop(stderr);
                }));
                drop(std::thread::spawn(move || {
                    let _ = child.wait();
                }));
            } else {
                // Should never happen
                let _ = control_and_errors.send(Operation::MessageDeliveryError(
                    "Failed to get stdin/stdout/stderr".to_string(),
                ));
            }
        }
}


