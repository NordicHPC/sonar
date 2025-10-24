#!/usr/bin/env bash
#
# Test these aspects of the process rollup algorithm:
#  - only siblings with the same name are rolled up
#
# We use an override to flag jobs as roll-uppable.
#
# This requires a (probably) 1.6x or later Rust/Cargo toolchain to build Sonar and `make` + any C89
# or later C compiler to build the C code.

source sh-helper
assert cargo cc

output=$(tmpfile rollup2)

make rollup-programs

echo " This takes about 10s"
./rollup2 3 &
sleep 3

SONARTEST_ROLLUP=1 cargo run -- ps --rollup --exclude-system-jobs > $output

nroll=$(jq '.data.attributes.jobs[].processes[]|select(.cmd=="rollupchild").rolledup' $output)
if (( nroll != 4 )); then
    fail "No matching rolledup=4"
fi

nroll2=$(jq '.data.attributes.jobs[].processes[]|select(.cmd=="rollupchild2").rolledup' $output)
if (( nroll2 != 3 )); then
    fail "No matching rolledup=3"
fi
echo " Ok"
