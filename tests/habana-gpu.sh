#!/usr/bin/env bash
#
# Check that Sonar can detect a Habana/Gaudi GPU if it ought to (based on info from the file
# system).
#
# This test must be run on a node with such a device to have any effect, hence will not be effective
# in the github runner.

source sh-helper
assert cargo jq

if [[ ! -e /sys/module/habanalabs ]]; then
    echo " No device"
    exit 0
fi

output=tmp/habana-gpu.tmp

source shared-gpu-smoketest

echo " OK"
