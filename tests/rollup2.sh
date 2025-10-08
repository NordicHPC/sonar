#!/usr/bin/env bash
#
# Test these aspects of the process rollup algorithm:
#  - only siblings with the same name are rolled up
#
# We use an override to flag jobs as roll-uppable.
#
# This requires a (probably) 1.6x or later Rust/Cargo toolchain to build Sonar and `make` + any C89
# or later C compiler to build the C code.

set -e

make all

echo " This takes about 10s"
./rollup2 3 &
sleep 3
output=$(SONARTEST_ROLLUP=1 cargo run -- ps --rollup --exclude-system-jobs)
# Grep will exit with code 1 if no lines are matched
matches1=$(grep -E ',cmd=rollupchild,.*,rolledup=4' <<< $output)
matches2=$(grep -E ',cmd=rollupchild2,.*,rolledup=3' <<< $output)

echo " Ok"
