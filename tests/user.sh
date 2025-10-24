#!/usr/bin/env bash
#
# Check that sonar can look up users.  There will be at least one process for the user: sonar.  I
# guess this could fail if there are multiple sonars running under the current user, which could be
# the case if we parallelize tests.  Oh well.

source sh-helper
assert cargo

n=$(cargo run -- ps | \
        jq ".data.attributes.jobs | map(select(.processes | any(.cmd == \"sonar\"))) | map(select(.user == \"$USER\")) | length")
if (( n != 1 )); then
    fail "User name lookup fails - n=$n"
fi

echo " Ok"
