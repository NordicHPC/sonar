#!/usr/bin/env bash
#
# Check that sonar can look up users.  There will be at least one process for the user: sonar.

source sh-helper

if (( $(cargo run -- ps --csv | grep ",user=$USER," | wc -l) == 0 )); then
    fail "User name lookup fails??"
fi

echo " Ok"
