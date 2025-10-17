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

make rollup-programs

echo " This takes about 10s"
./rollup 3 &
sleep 3
output=$(SONARTEST_ROLLUP=1 cargo run -- ps --rollup --exclude-system-jobs --csv)
set +e
matches=$(grep ,cmd=rollup, <<< $output)
rolled=$(grep ,rolledup=1 <<< $matches)
rolled2=$(grep ,rolledup= <<< $matches)
set -e
nmatch=$(wc -l <<< $matches)
if (( nmatch != 23 )); then
    fail "Bad number of matching lines, want 23, got $nmatch"
fi
nroll=$(wc -l <<< $rolled)
if (( nroll != 8 )); then
    fail "Bad number of rolled-up lines with value 1, want 8, got $nroll"
fi
nroll2=$(wc -l <<< $rolled2)
if (( nroll2 != 8 )); then
    fail "Bad number of rolled-up lines - some have a value other than 1"
fi
echo " Ok"
