#!/usr/bin/env bash
#
# Check that sonar can look up users.  There will be at least one process for the user: sonar.

source sh-helper
assert cargo

if ! cargo run -- ps --csv | grep -q ",user=$USER,"; then
    fail "User name lookup fails??"
fi

echo " Ok"
