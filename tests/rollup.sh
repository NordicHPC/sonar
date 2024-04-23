#!/bin/bash
#
# Test these aspects of the process rollup algorithm:
#  - only leaf processes are rolled up
#  - only siblings of the same parent are rolled up
#
# To do this on a non-slurm system we run --rollup --batchless with an override to allow that.
#
# This requires a (probably) 1.6x or later Rust/Cargo toolchain to build Sonar and `make` + any C89
# or later C compiler to build the C code.

set -e

( cd .. ; cargo build )
make --quiet

echo "This takes about 10s"
./rollup 3 &
sleep 3
output=$(SONARTEST_ROLLUP=1 ../target/debug/sonar ps --rollup --batchless --exclude-system-jobs)
matches=$(grep ,cmd=rollup, <<< $output)
rolled=$(grep ,rolledup=1 <<< $matches)
rolled2=$(grep ,rolledup= <<< $matches)
if [[ $(wc -l <<< $matches) != 23 ]]; then
    echo "Bad number of matching lines"
    exit 1
fi
if [[ $(wc -l <<< $rolled) != 8 ]]; then
    echo "Bad number of rolled-up lines with value 1"
    exit 1
fi
if [[ $(wc -l <<< $rolled2) != 8 ]]; then
    echo "Bad number of rolled-up lines - some have a value other than 1"
    exit 1
fi
echo Success
