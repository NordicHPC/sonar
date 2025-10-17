#!/usr/bin/env bash
#
# Check that sonar reports the correct hostname

source sh-helper

if (( $(cargo run -- ps --csv | head -n 1 | grep ",host=$(hostname)," | wc -l) == 0 )); then
    fail "Wrong hostname??"
fi

echo " Ok"
