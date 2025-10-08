#!/usr/bin/env bash
#
# Test the cpu_util computation by spinning up a process that will grab one core at 100% for a while
# and run sonar ps meanwhile.

source sh-helper
assert_jq

make pincpu
cargo build

# Spin up and wait for it to get up to speed

./pincpu 10 > /dev/null &
sleep 4

# Run sonar and grab the value and test it.
#
# The CSV format does not have cpu_util so go to JSON.
#
# We want the cpu_util field from the process with cmd=pincpu (ideally where the enclosing job has
# user=$LOGNAME).  Ignore the bit with $LOGNAME for now.
#
# The cpu_util is floating point, so we need to round it.

output=$(cargo run -- ps --exclude-system-jobs --json --cluster my.cluster)
util=$(jq '.data.attributes.jobs[].processes[]|select(.cmd=="pincpu").cpu_util|ceil' <<< $output)

if (( util < 75 || util > 125 )); then
    fail "Unlikely CPU utilization $util"
fi

echo " Ok"
