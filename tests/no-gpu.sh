#!/usr/bin/env bash
#
# First check that `sonar sysinfo` will detect no GPUs if there are no GPUs.
#
# Next check and that there are no GPU fields in the output from `sonar ps`.

# Add other GPU types here when we add support for them, the tests below should start failing when
# that happens.
set -e
if [[ -e /sys/module/amdgpu || -e /sys/module/nvidia || -e /sys/module/i915 || -e /sys/module/habanalabs ]]; then
    echo " GPUs detected"
    exit 0
fi

output=$(cargo run -- sysinfo)
numcards=$(jq .gpu_cards <<< $output)
if (( numcards != 0 )); then
    echo "Bad output from jq: <$numcards> should be zero"
    exit 1
fi

# TODO: Once we have JSON output, use that here!  The CSV matching is very crude and there's a small
# chance that it will have a false positive on sufficiently perverse command names.

output=$(cargo run -- ps --load)
if [[ $output =~ ,gpu[%a-z_-]+= ]]; then
    echo "Bad output: unexpected GPU fields in output on non-gpu system"
    exit 1
fi

echo " OK"
