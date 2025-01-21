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

# Test that sysinfo finds the cards.  This is also sufficient to test that the GPU SMI library has
# been found and is loaded.

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

# Run ps once with --load to trigger the collection of gpu utilization data.  This is just a
# smoketest: we're not guaranteed that anything is running on the GPUs and can't really guarantee
# that there is any output.  But on the AMD GPU there is always *some* output.
#
# TODO: This will be cleaner once we have json output.

output=$(../target/debug/sonar ps --load --exclude-system-jobs)
infos=$(grep -E 'gpuinfo=.*fan%=.*tempc=.*' <<< $output)
lines=$(wc -l <<< $infos)
if (( $lines != 1 )); then
    echo "Number of matching output lines should be 1"
    exit 1
fi

echo "OK"
