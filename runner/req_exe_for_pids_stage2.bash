#!/usr/bin/env bash
#
# Test code for the REQ_EXE_FOR_PIDS function of the sonar-runner.  Stage 2.  Do not run this
# manually, it is run by stage 1.

if [[ $(whoami) != "root" ]]; then
    echo "Must run as root"
    exit 1
fi

# test_exe_pids.txt must be a comma-separated list of PIDs on one line
# test_exe_id.txt must be on the form username groupname
if [[ ! -f test_exe_pids.txt || ! -f test_exe_id.txt || ! -f test_exe_exe.txt ]]; then
    echo "Missing required test_exe_....txt file"
    exit 1
fi
ASSUMED_ID=$(cat test_exe_id.txt)
EXECUTABLE=$(cat test_exe_exe.txt)
REQ_EXE_FOR_PIDS=$(cat test_exe_pids.txt) ./build/sonar-daemon-runner-test "$EXECUTABLE" test.cfg $ASSUMED_ID > test_exe_result.txt
