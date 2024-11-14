#!/usr/bin/env bash
#
# Test that we can emit per-gpu load data properly.

set -e

# Currently testing this only on nvidia.
if [[ ! -e /sys/module/nvidia ]]; then
    exit 0
fi

( cd .. ; cargo build )

# The field is going to be there because cards always have some non-default data (fan speeds,
# performance state, power, clocks).

loadlines=$(../target/debug/sonar ps --load | grep -E ',"?gpuinfo=' | wc -l)
if [[ $loadlines -ne 1 ]]; then
    echo "Did not emit gpuinfo data properly - not exactly 1: $loadlines"
    exit 1
fi

loadlines=$(../target/debug/sonar ps | grep -E ',"?gpuinfo=' | wc -l)
if [[ $loadlines -ne 0 ]]; then
    echo "Did not emit gpuinfo data properly - not exactly 0: $loadlines"
    exit 1
fi
