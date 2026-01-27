#!/usr/bin/env bash
#
# Test that errors are signalled properly for failing to find config file or failing to parse it.

source sh-helper
assert cargo

log=$(tmpfile daemon-read-config-log)
ini=$(tmpfile daemon-read-config-ini)

cargo run -- daemon no-such-ini-file.txt > /dev/null 2> $log

if ! grep -q -E 'ERROR.*Daemon.*No such file' $log; then
    fail "Expected error message in log file"
fi

cat > $ini <<EOF
bad bad bad
EOF

cargo run -- daemon $ini > /dev/null 2> $log

if ! grep -q -E 'ERROR.*Daemon.*Illegal property definition' $log; then
    fail "Expected error message in log file"
fi

echo " Ok"
