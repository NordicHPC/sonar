#!/bin/bash
#
# Test the interrupt logic in sonar.  A TERM or HUP signal can be sent and the process will exit in
# an orderly way with a message on stderr.

set -e
echo "This takes about 15s"
( cd .. ; cargo build )
rm -f interrupt.output.txt
SONARTEST_WAIT_INTERRUPT=1 ../target/debug/sonar ps 2> interrupt.output.txt &
bgpid=$!
sleep 3
kill -TERM $bgpid
sleep 10
if [[ $(cat interrupt.output.txt) != 'Interrupt flag was set!' ]]; then
    echo "Unexpected output!"
    exit 1
fi
rm -f interrupt.output.txt

