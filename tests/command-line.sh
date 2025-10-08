#!/usr/bin/env bash
#
# Check that command line parsing is somewhat sane.

source sh-helper

# Allow both forms of argument syntax
cargo run -- ps --exclude-users=root,$LOGNAME > /dev/null
cargo run -- ps --exclude-users root,$LOGNAME > /dev/null

# Test all arguments in combination without --rollup
cargo run -- \
      ps \
      --min-cpu-percent 0.5 \
      --min-mem-percent 1.8 \
      --min-cpu-time 10 \
      --exclude-system-jobs \
      --exclude-users root \
      --exclude-commands emacs \
      --lockdir . \
      > /dev/null

# Test all arguments in combination with --rollup
cargo run -- \
      ps \
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
output=$(cargo run -- ps --zappa 2>&1)
exitcode=$?
set -e
if (( exitcode != 2 )); then
    fail "Failed to reject unknown argument"
fi

# Signal error with code 2 for invalid arguments: missing string
set +e
output=$(cargo run -- ps --lockdir 2>&1)
exitcode=$?
set -e
if (( exitcode != 2 )); then
    fail "Lockdir should require an argument value"
fi

# Signal error with code 2 for invalid arguments: bad number
set +e
output=$(cargo run -- ps --min-cpu-time 7hello 2>&1)
exitcode=$?
set -e
if (( exitcode != 2 )); then
    fail "min-cpu-time should require an integer argument value"
fi

echo " Ok"
