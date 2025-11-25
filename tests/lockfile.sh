#!/usr/bin/env bash
#
# Test the lock file logic in sonar.  Sonar creates a lock file when it runs; a subsequent run that
# starts while the lock file exists will terminate immediately with a log message.

source sh-helper
assert cargo

logfile=$(tmpfile lockfile.output)
rm -f sonar-lock.*

echo " This takes about 15s"
SONARTEST_WAIT_LOCKFILE=1 cargo run -- ps --lockdir . > /dev/null &
bgpid=$!
# Wait for the first process to get going
sleep 3
cargo run -- ps --lockdir . 2> $logfile
if ! grep -q -E "WARN.*Lockfile present, exiting" $logfile; then
    fail "Unexpected output!"
fi
# Wait for the first process to exit
sleep 10

echo " Ok"
