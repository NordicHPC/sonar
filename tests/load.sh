#!/usr/bin/env bash
#
# Test that we can emit per-cpu load data properly.

source sh-helper

loadlines=$(cargo run -- ps --load --csv | grep ',load=' | wc -l)
if (( loadlines != 1 )); then
    fail "Did not emit load data properly - not exactly 1"
fi

loadlines=$(cargo run -- ps --csv | grep ',load=' | wc -l)
if (( loadlines != 0 )); then
    fail "Did not emit load data properly - not exactly 0"
fi

echo " Ok"
