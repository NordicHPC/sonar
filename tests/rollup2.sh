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

output=tmp/rollup2.tmp

make rollup-programs

echo " This takes about 10s"
./rollup2 3 &
sleep 3
SONARTEST_ROLLUP=1 cargo run -- ps --rollup --exclude-system-jobs --csv > $output
if ! grep -q -E ',cmd=rollupchild,.*,rolledup=4' $output; then
    fail "No matching rolledup=4"
fi
if ! grep -q -E ',cmd=rollupchild2,.*,rolledup=3' $output; then
    fail "No matching rolledup=3"
fi
echo " Ok"
