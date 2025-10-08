#!/usr/bin/env bash
#
# Test that we can emit per-gpu load data properly.

source sh-helper

# Currently testing this only on nvidia.
if [[ ! -e /sys/module/nvidia ]]; then
    echo " No gpu"
    exit 0
fi

# The field is going to be there because cards always have some non-default data (fan speeds,
# performance state, power, clocks).

loadlines=$(cargo run -- ps --load | grep -E ',"?gpuinfo=' | wc -l)
if (( loadlines != 1 )); then
    fail "Did not emit gpuinfo data properly - not exactly 1: $loadlines"
fi

loadlines=$(cargo run -- ps | grep -E ',"?gpuinfo=' | wc -l)
if (( loadlines != 0 )); then
    fail "Did not emit gpuinfo data properly - not exactly 0: $loadlines"
fi

echo " Ok"
