# Testing HTTP exfiltration

You need two shells, D and S.

To test the HTTP path manually, do this in shell D:

- grab https://github.com/lars-t-hansen/util.git
- in the httpdump/ subdirectory, `go build` && `./httpdump -p 4444`

Then in shell S, in the present directory, `cargo run -- daemon sonar.cfg`.

Then watch the traffic in D and S.  In S you should see log messages about data being generated and
uploaded.  In D you should see data about messages and data being received.
