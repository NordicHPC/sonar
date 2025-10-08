#!/usr/bin/env bash
#
# Test that we can emit per-cpu load data properly.

set -e

loadlines=$(cargo run -- ps --load | grep ',load=' | wc -l)
if (( loadlines != 1 )); then
    echo "Did not emit load data properly - not exactly 1"
    exit 1
fi

loadlines=$(cargo run -- ps | grep ',load=' | wc -l)
if (( loadlines != 0 )); then
    echo "Did not emit load data properly - not exactly 0"
    exit 1
fi

echo " Ok"
