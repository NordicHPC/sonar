#!/usr/bin/env bash
#
# Test these aspects of the process rollup algorithm:
#  - only siblings with the same name are rolled up
#
# To do this on a non-slurm system we run --rollup --batchless with an override to allow that.
#
# This requires a (probably) 1.6x or later Rust/Cargo toolchain to build Sonar and `make` + any C89
# or later C compiler to build the C code.

set -e

( cd .. ; cargo build )
make --quiet

echo " This takes about 10s"
./rollup2 3 &
sleep 3
output=$(SONARTEST_ROLLUP=1 ../target/debug/sonar ps --rollup --batchless --exclude-system-jobs)
# Grep will exit with code 1 if no lines are matched
matches1=$(grep -E ',cmd=rollupchild,.*,rolledup=4' <<< $output)
matches2=$(grep -E ',cmd=rollupchild2,.*,rolledup=3' <<< $output)
echo " OK"
