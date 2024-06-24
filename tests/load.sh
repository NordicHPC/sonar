#!/usr/bin/env bash
#
# Test that we can emit per-cpu load data properly.

set -e

( cd .. ; cargo build )
loadlines=$(../target/debug/sonar ps --load | grep ',load=' | wc -l)
if [[ $loadlines -ne 1 ]]; then
    echo "Did not emit load data properly - not exactly 1"
    exit 1
fi

loadlines=$(../target/debug/sonar ps | grep ',load=' | wc -l)
if [[ $loadlines -ne 0 ]]; then
    echo "Did not emit load data properly - not exactly 0"
    exit 1
fi
