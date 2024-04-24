#!/usr/bin/env bash
#
# Test the lock file logic in sonar.  Sonar creates a lock file when it runs; a subsequent run that
# starts while the lock file exists will terminate immediately with a log message.

set -e
logfile=lockfile.output.txt

echo "This takes about 15s"
( cd .. ; cargo build )
rm -f $logfile sonar-lock.*
SONARTEST_WAIT_LOCKFILE=1 ../target/debug/sonar ps --lockdir . > /dev/null &
bgpid=$!
# Wait for the first process to get going
sleep 3
../target/debug/sonar ps --lockdir . 2> $logfile
if [[ $(cat $logfile) != 'Info: Lockfile present, exiting' ]]; then
    echo "Unexpected output!"
    exit 1
fi
# Wait for the first process to exit
sleep 10
# Do not delete the lockfile here, that should be handled by the first sonar process
rm -f $logfile
