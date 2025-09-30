#!/usr/bin/env bash
#
# Test the lock file logic in sonar.  Sonar creates a lock file when it runs; a subsequent run that
# starts while the lock file exists will terminate immediately with a log message.

set -e
mkdir -p tmp
logfile=tmp/lockfile.output.txt
rm -f $logfile sonar-lock.*

echo " This takes about 15s"
SONARTEST_WAIT_LOCKFILE=1 cargo run -- ps --lockdir . > /dev/null &
bgpid=$!
# Wait for the first process to get going
sleep 3
cargo run -- ps --lockdir . 2> $logfile
if [[ $(tail -n 1 $logfile) != 'Info: Lockfile present, exiting' ]]; then
    echo "Unexpected output!"
    exit 1
fi
# Wait for the first process to exit
sleep 10
