#!/usr/bin/env bash
#
# Test code for the REQ_EXE_FOR_PIDS function of the sonar-runner.  Stage 2.  Do not run this
# manually, it is run by stage 1.

if [[ $(whoami) != "root" ]]; then
    echo "Must run as root"
    exit 1
fi

# test_pids.txt must be a comma-separated list of PIDs on one line
# test_id.txt must be on the form username groupname
if [[ ! -f test_pids.txt || ! -f test_id.txt ]]; then
    echo "Expected test_pids.txt and test_id.txt"
    exit 1
fi
ASSUMED_ID=$(cat test_id.txt)
EXECUTABLE=$(cat test_exe.txt)
REQ_EXE_FOR_PIDS=$(cat test_pids.txt) ./sonar-daemon-runner-test "$EXECUTABLE" test.cfg $ASSUMED_ID > test_result.txt

