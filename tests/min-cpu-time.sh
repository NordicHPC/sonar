#!/usr/bin/env bash
#
# Test that the --min-cpu-time switch works.

source sh-helper
assert jq cargo

result=$(cargo run -- ps --min-cpu-time 5 | jq '.data.attributes.jobs[].processes[].cpu_time | select(. < 5)')
if [[ -n $result ]]; then
    fail "CPU time filtering did not work: should have none under 5"
fi

# At least sonar will have < 5s, most processes will have "null" because the field is zero and is
# not reported.
result=$(cargo run -- ps | jq '.data.attributes.jobs[].processes[].cpu_time | select(. < 5)')
if [[ -z $result ]]; then
    fail "CPU time filtering did not work: should have some under 5"
fi

echo " Ok"
