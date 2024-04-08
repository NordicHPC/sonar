#!/bin/bash
#
# Check that command line parsing is somewhat sane.

set -e
( cd ..; cargo build )

# Allow both forms of argument syntax
../target/debug/sonar ps --exclude-users=root,$LOGNAME > /dev/null
../target/debug/sonar ps --exclude-users root,$LOGNAME > /dev/null

# Allow all these arguments
../target/debug/sonar ps \
                      --batchless \
                      --rollup \
                      --min-cpu-percent 0.5 \
                      --min-mem-percent 1.8 \
                      --min-cpu-time 10 \
                      --exclude-system-jobs \
                      --exclude-users root \
                      --exclude-commands emacs \
                      --lockdir . \
                      > /dev/null

# Signal error with code 2 for unknown arguments
set +e
output=$(../target/debug/sonar ps --zappa 2>&1)
exitcode=$?
set -e
if [[ $exitcode != 2 ]]; then
    echo "Failed to reject unknown argument"
    exit 1
fi

# Signal error with code 2 for invalid arguments: missing string
set +e
output=$(../target/debug/sonar ps --lockdir 2>&1)
exitcode=$?
set -e
if [[ $exitcode != 2 ]]; then
    echo "Lockdir should require an argument value"
    exit 1
fi

# Signal error with code 2 for invalid arguments: bad number
set +e
output=$(../target/debug/sonar ps --min-cpu-time 7hello 2>&1)
exitcode=$?
set -e
if [[ $exitcode != 2 ]]; then
    echo "min-cpu-time should require an integer argument value"
    exit 1
fi
