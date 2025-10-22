#!/usr/bin/env bash
#
# Check that sonar reports the correct hostname

source sh-helper
assert cargo

if ! cargo run -- ps --csv | head -n 1 | grep -q ",host=$(hostname),"; then
    fail "Wrong hostname??"
fi

echo " Ok"
