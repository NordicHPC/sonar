#!/usr/bin/env bash
#
# Test these aspects of the process rollup algorithm:
#  - only leaf processes are rolled up
#  - only siblings of the same parent are rolled up
#
# We use an override to flag jobs as roll-uppable.
#
# This requires a (probably) 1.6x or later Rust/Cargo toolchain to build Sonar and `make` + any C89
# or later C compiler to build the C code.

source sh-helper
assert cargo cc

output=$(tmpfile rollup)

make rollup-programs

echo " This takes about 10s"
./rollup 3 &
sleep 3

SONARTEST_ROLLUP=1 cargo run -- ps --rollup --exclude-system-jobs > $output

nmatch=$(jq '[.data.attributes.jobs[].processes[]] | map(select(.cmd=="rollup")) | length' $output)
if (( nmatch != 23 )); then
    fail "Bad number of matching lines, want 23, got $nmatch"
fi

nroll=$(jq '[.data.attributes.jobs[].processes[]] | map(select(.cmd=="rollup")) | map(select(.rolledup==1)) | length' $output)
if (( nroll != 8 )); then
    fail "Bad number of rolled-up lines with value 1, want 8, got $nroll"
fi

nroll2=$(jq '[.data.attributes.jobs[].processes[]] | map(select(.cmd=="rollup")) | map(select(has("rolledup"))) | length' $output)
if (( nroll2 != nroll )); then
    fail "Bad number of rolled-up lines - some have a value other than 1"
fi

# This is true for one-shot mode but not for daemon mode
roll3=$(jq -r '[.data.attributes.jobs[].processes[]] | map(select(.cmd=="rollup" and .pid!=null and .rolledup!=null))' $output)
if [[ $roll3 != '[]' ]]; then
    fail "Should have seen no rolledup lines with nonzero pid"
fi

echo " Ok"
exit 0
