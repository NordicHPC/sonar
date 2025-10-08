#!/usr/bin/env bash
#
# Test the ps interrupt logic in sonar.  A TERM or HUP signal can be sent and the process will exit
# in an orderly way with a message on stderr.

source sh-helper
echo " This takes about 20s"

mkdir -p tmp
output=tmp/interrupt.output.txt
rm -f $output

SONARTEST_WAIT_INTERRUPT=1 cargo run -- ps 2> $output &
bgpid=$!
sleep 10
kill -TERM $bgpid
sleep 10
if [[ $(tail -n 1 $output) != 'Info: Interrupt flag was set!' ]]; then
    fail "Unexpected output!"
fi

echo " Ok"
