// Low-level HTTP upload logic.  HttpUploader is a simple back-end that will HTTP POST some data to
// an endpoint, while handling proxies, timeouts, retries, etc.

use std::cmp::min;
use std::io::{Read, Write};

use crossbeam::channel;

pub struct Credential {
    user: String,
    passwd: String,
    sloppy: bool,
}

impl Credential {
    #[allow(dead_code)]
    pub fn from_user_passwd(user: &str, passwd: &str, sloppy: bool) -> Credential {
        Credential {
            user: user.to_string(),
            passwd: passwd.to_string(),
            sloppy,
        }
    }

    fn netrc_file(&self) -> Option<tempfile::NamedTempFile> {
        if self.passwd == "" {
            None
        } else {
            // I don't like this but it's the right API.  With curl, we'd like not to pass
            // credentials on the command line.  (Curl cleans them out but a determined attacker
            // with the ability to run jobs on many nodes could get lucky and observe the full
            // command line with ps and see the password.)  The "easy" way around that is to create
            // a netrc file containing the credentials and pass the name of that file.  For that, we
            // need a named temp file.
            //
            // Named temp files are surprisingly hard.  We've promised that Sonar only writes to the
            // temp dir, which on Linux is shared, and subject to periodic file cleaning by systemd
            // (so long-lived named temp files require maintenance in the form of replacements or
            // refreshes, and good luck testing that functionality reliably).  Hence short-lived
            // temp files in the shared temp dir are the easy path.  The file will be readable only
            // to the Sonar user, but the shared temp directory is writeable to everyone, so there's
            // at least the possibility of a malicious user deleting or replacing the file
            // before/while curl is running, leading to a DoS situation (though no leaked
            // credentials).  This malicious user needs elevated privileges I think, since files in
            // the temp dir owned by X aren't deletable by Y (at least on my system), so the problem
            // seems remote on current evidence.
            //
            // Anyway, what we do here is create a temp netrc file per request, that is deleted once
            // curl exits.  The longer-term fix is to get rid of the use of curl.  The contents of
            // the file are `default login {cluster} password {pass}`, so we don't have to parse the
            // upload URL, a small win.
            match tempfile::Builder::new()
                .prefix("s")
                .rand_bytes(10)
                .suffix(".txt")
                .tempfile()
            {
                Ok(f) => match std::fs::write(
                    f.path(),
                    format!("default login {} password {}", self.user, self.passwd).as_bytes(),
                ) {
                    Ok(_) => Some(f),
                    Err(_) => None,
                },
                Err(_) => None,
            }
        }
    }

    fn user_passwd(&self) -> Option<(String, String)> {
        if self.passwd == "" || !self.sloppy {
            None
        } else {
            Some((self.user.clone(), self.passwd.clone()))
        }
    }
}

pub struct HttpUploader<'a> {
    curl_cmd: &'a str,
    http_proxy: &'a str,
    retry_count: i32,
}

impl<'a> HttpUploader<'a> {
    // Create the uploader.  Using the provided curl_cmd, it will POST data to a URL, possibly via
    // the given http_proxy, retrying periodically until timeout seconds have elapsed.  After
    // creating the uploader, call start() to fork off a curl process that will perform the upload
    // to a particular URL; this is represented by the returned HttpUploadStream.  Call put()
    // repeatedly on the stream to send data, and finally end() to close down the upload.  It is
    // possible to call start() repeatedly on the same uploader with different URLs, it will create
    // independent subprocesses and threads to handle the additional uploads, all of which will be
    // performed concurrently.
    pub fn new(curl_cmd: &'a str, http_proxy: &'a str, mut timeout: u64) -> HttpUploader<'a> {
        // Curl will retry for 1s, 2s, 4s, ..., 10m and then stick to 10m
        let mut retry_count = 0;
        let mut next = 1;
        while timeout > 0 {
            timeout -= min(next, timeout);
            next = min(600, next * 2);
            retry_count += 1;
        }
        HttpUploader {
            curl_cmd,
            http_proxy,
            retry_count,
        }
    }

    // Start an upload to a target address, see doc at new().
    pub fn start(
        &self,
        url: &str,
        mime_type: &str,
        cred: &Option<Credential>,
    ) -> Result<HttpUploadStream, String> {
        // For now, the logic here is that to send a data package we fork off a curl and make it
        // send the output and handle retries, it will automatically pick up proxy settings from the
        // environment.  The main thread does not wait for it to finish but spins up threads to
        // handle its stdin/stdout/stderr and the final wait.
        let mut args = vec![
            "--silent".to_string(),
            "--show-error".to_string(),
            "--data-binary".to_string(),
            "@-".to_string(),
            "-H".to_string(),
            "Content-Type: ".to_string() + mime_type,
        ];
        if self.retry_count > 0 {
            args.push("--retry".to_string());
            args.push(format!("{}", self.retry_count));
            args.push("--retry-connrefused".to_string());
        }
        let tempfile = match cred {
            None => None,
            Some(c) => {
                if let Some(tempfile) = c.netrc_file() {
                    if let Some(s) = tempfile.path().to_str() {
                        args.push("--netrc-file".to_string());
                        args.push(s.to_string());
                        Some(tempfile)
                    } else {
                        None
                    }
                } else if let Some((user, pass)) = c.user_passwd() {
                    args.push("--user".to_string());
                    args.push(user + ":" + &pass);
                    None
                } else {
                    None
                }
            }
        };
        args.push(url.to_string());

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
                    Ok(HttpUploadStream::start(
                        child, stdin, stdout, stderr, tempfile,
                    ))
                } else {
                    // Should never happen, probably
                    Err("Failed to get stdin/stdout/stderr".to_string())
                }
            }
            Err(e) => {
                // Note, tests depend on this error message
                Err(format!("Failed to launch curl: {:?}", e))
            }
        }
    }
}

pub struct HttpUploadStream {
    sending: channel::Sender<Option<String>>,
}

impl Drop for HttpUploadStream {
    fn drop(&mut self) {
        let _ = self.end();
    }
}

impl HttpUploadStream {
    fn start(
        mut child: std::process::Child,
        mut stdin: std::process::ChildStdin,
        mut stdout: std::process::ChildStdout,
        mut stderr: std::process::ChildStderr,
        tempfile: Option<tempfile::NamedTempFile>,
    ) -> HttpUploadStream {
        let (sending, receiving) = channel::unbounded::<Option<String>>();

        // Writer thread

        drop(std::thread::spawn(move || {
            // get byte blobs from stream and write them to stdin.  There must be a signal for this
            // to exit, otherwise nothing will work.  So end() should send an empty array or something.
            while let Ok(Some(payload)) = receiving.recv() {
                if let Err(err) = stdin.write_all(payload.as_bytes()) {
                    log::debug!("Failed to write payload: {:?}", err);
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
            // The netrc tempfile has credentials and must live this long
            drop(tempfile);
        }));

        HttpUploadStream { sending }
    }

    pub fn put_string(&self, s: String) {
        let _ = self.sending.send(Some(s));
    }

    pub fn end(&self) -> Result<(), String> {
        let _ = self.sending.send(None);
        Ok(())
    }
}
