#!/usr/bin/env bash
#
# Check that `sonar sysinfo` can detect an AMD GPU if it ought to (based on info from the file
# system).  This test must be run on a node with such a device to have any effect, hence will not be
# effective in the github runner.
#
# Requirement: the `jq` utility.

set -e
if [[ ! -e /sys/module/amdgpu ]]; then
    echo "No device"
    exit 0
fi

( cd .. ; cargo build )
output=$(../target/debug/sonar sysinfo)
numcards=$(jq .gpu_cards <<< $output)
if [[ ! ( $numcards =~ ^[0-9]+$ ) ]]; then
    echo "Bad output from jq: <$numcards>"
    exit 1
fi
if (( $numcards == 0 )); then
    echo "Number of cards should be nonzero"
    exit 1
fi
echo "OK"
