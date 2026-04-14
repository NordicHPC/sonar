use std::io::{Read, Write};
use std::cmp::min;

use crossbeam::channel;

// The abstraction here will be a "http poster" that encapsulates the sending method and a bunch of
// the other stuff, and into which we can pump a bunch of data.  Then it does not matter if that
// uses curl or some other method.  The http poster can be shared between the kafka code and the
// pure http POST code, and has the same general shape as the kafka poster code.

pub struct HttpUploader<'a> {
    curl_cmd: &'a str,
    api_endpoint: &'a str,
    http_proxy: &'a str,
    timeout: u64,
}

impl<'a> HttpUploader<'a> {
    pub fn new(curl_cmd: &'a str, api_endpoint: &'a str, http_proxy: &'a str, timeout: u64) -> HttpUploader<'a> {
        HttpUploader{
            curl_cmd,
            api_endpoint,
            http_proxy,
            timeout,
        }
    }

    pub fn start(&self) -> Result<HttpUploadStream, String> {
        // For now, the logic here is that to send a data package we fork off a curl and make it
        // send the output and handle retries, it will automatically pick up proxy settings from the
        // environment.  The main thread does not wait for it to finish but spins up threads to
        // handle its stdin/stdout/stderr and the final wait.

        // Curl will retry for 1s, 2s, 4s, ..., 10m and then stick to 10m
        let mut retry_count = 0;
        let mut next = 1;
        let mut timeout = self.timeout;
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
        args.push(self.api_endpoint.to_string());

        // Really want to merge stdout and stderr if we can.
        let mut cmd = std::process::Command::new(self.curl_cmd);
        log::debug!("Curl: {} {:?}", self.curl_cmd, args);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if self.http_proxy != "" {
            cmd.env("http_proxy", self.http_proxy)
                .env("https_proxy", self.http_proxy);
        }
        match cmd.spawn() {
            Ok(mut child) => {
                if let (Some(stdin), Some(stdout), Some(stderr)) =
                    (child.stdin.take(), child.stdout.take(), child.stderr.take())
                {
                    Ok(HttpUploadStream::start(child, stdin, stdout, stderr))
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

pub struct HttpUploadStream {
    sending: channel::Sender<Option<String>>,
}

// FIXME: This needs a drop() thing that calls end().

impl HttpUploadStream {
    fn start(
        mut child: std::process::Child,
        mut stdin: std::process::ChildStdin,
        mut stdout: std::process::ChildStdout,
        mut stderr: std::process::ChildStderr,
    ) -> HttpUploadStream {
        let (sending, receiving) = channel::unbounded::<Option<String>>();

        // Writer thread

        drop(std::thread::spawn(move || {
            // get byte blobs from stream and write them to stdin.  There must be a signal for this
            // to exit, otherwise nothing will work.  So end() should send an empty array or something.
            loop {
                match receiving.recv() {
                    Ok(Some(payload)) => {
                        if let Err(err) = stdin.write_all(payload.as_bytes()) {
                            log::debug!("Failed to write payload: {:?}", err);
                        }
                    }
                    Ok(None) | Err(_) => {
                        break;
                    }
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
        // to fork off these threads until writing has completed.  We could maybe combine the
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

        HttpUploadStream{sending}
    }

    pub fn put_string(&self, s: String) {
        let _ = self.sending.send(Some(s));
    }

    pub fn end(&self) -> Result<(), String> {
        let _ = self.sending.send(None);
        Ok(())
    }
}
