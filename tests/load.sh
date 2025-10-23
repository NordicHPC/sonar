#!/usr/bin/env bash
#
# Test that we can emit per-cpu load data properly.

source sh-helper
assert cargo

set +e
loaded=$(cargo run -- ps --load --csv | grep -c ',load=')
set -e
if (( $loaded != 1 )); then
    fail "Did not emit load data properly - not exactly 1: $loaded"
fi

if cargo run -- ps --csv | grep -q ',load='; then
    fail "Did not emit load data properly - not exactly 0"
fi

echo " Ok"
