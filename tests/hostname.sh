#!/usr/bin/env bash
#
# Check that sonar reports the correct hostname

set -e
if [[ $(cargo run -- ps | head -n 1 | grep ",host=$(hostname)," | wc -l) == 0 ]]; then
    echo "Wrong hostname??"
    exit 1
fi
