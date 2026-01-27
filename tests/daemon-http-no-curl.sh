#!/usr/bin/env bash

source sh-helper
assert cargo

inifile=$(tmpfile daemon-http-no-curl-ini)
logfile=$(tmpfile daemon-http-no-curl-log)

cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=node

[programs]
curl-command = /bin/no-such-curl

[debug]
# oneshot does not work properly but this is ok
time-limit = 5s

[kafka]
rest-endpoint = no.such.host:101010
sending-window = 1s
sasl-password = foobar

[sysinfo]
cadence=1s
EOF

cargo run -- daemon $inifile > /dev/null 2> $logfile

if ! grep -q -E 'ERROR.*Daemon.*Failed to launch curl.*NotFound' $logfile; then
    fail "Expected error for nonexistent curl"
fi


