#!/usr/bin/env bash
#
# Test code for the REQ_ENV_VARS_FOR_PIDS function of the sonar-runner.  Stage 2.  Do not run this
# manually, it is run by stage 1.

if [[ $(whoami) != "root" ]]; then
    echo "Must run as root"
    exit 1
fi

# test_ev4p_pids.txt must be a comma-separated list of PIDs on one line
# test_ev4p_id.txt must be on the form username groupname
if [[ ! -f test_ev4p_pids.txt || ! -f test_ev4p_id.txt || ! -f test_ev4p_exe.txt ]]; then
    echo "Missing required test_ev4p_....txt file"
    exit 1
fi
ASSUMED_ID=$(cat test_ev4p_id.txt)
EXECUTABLE=$(cat test_ev4p_exe.txt)
REQ_ENV_VARS_FOR_PIDS=$(cat test_ev4p_pids.txt) ./build/sonar-daemon-runner-test "$EXECUTABLE" test.cfg $ASSUMED_ID > test_ev4p_result.txt
