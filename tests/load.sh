#!/usr/bin/env bash
#
# Test that we can emit per-cpu load data properly.

source sh-helper
assert cargo

loaded=$(cargo run -- ps --load | jq '.data.attributes.system.cpus | length')
if (( loaded == 0 )); then
    fail "Did not emit load data properly - should be positive: $loaded"
fi

loaded=$(cargo run -- ps | jq '.data.attributes.system.cpus | length')
if (( loaded != 0 )); then
    fail "Did not emit load data properly - should be zero: $loaded"
fi

echo " Ok"
