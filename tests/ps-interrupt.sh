#!/usr/bin/env bash
#
# Test the ps interrupt logic in sonar.  A TERM or HUP signal can be sent and the process will exit
# in an orderly way with a message on stderr.

source sh-helper
assert cargo

echo " This takes about 20s"

output=$(tmpfile interrupt.output)

SONARTEST_WAIT_INTERRUPT=1 RUST_LOG=debug cargo run -- ps 2> $output &
bgpid=$!
sleep 10
kill -TERM $bgpid
sleep 10
if ! grep -q -E "DEBUG.*Interrupt flag was set!" $output; then
    fail "Unexpected output!"
fi

echo " Ok"
