#!/usr/bin/env bash
#
# Test the ps interrupt logic in sonar.  A TERM or HUP signal can be sent and the process will exit
# in an orderly way with a message on stderr.

set -e
echo " This takes about 20s"
rm -f interrupt.output.txt
SONARTEST_WAIT_INTERRUPT=1 cargo run -- ps 2> interrupt.output.txt &
bgpid=$!
sleep 10
kill -TERM $bgpid
sleep 10
if [[ $(tail -n 1 interrupt.output.txt) != 'Info: Interrupt flag was set!' ]]; then
    echo "Unexpected output!"
    exit 1
fi
rm -f interrupt.output.txt

