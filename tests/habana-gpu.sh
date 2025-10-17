#!/usr/bin/env bash
#
# Check that `sonar sysinfo` can detect a Habana/Gaudi GPU if it ought to (based on info from the
# file system).  This test must be run on a node with such a device to have any effect, hence will
# not be effective in the github runner.

source sh-helper
assert_jq

if [[ ! -e /sys/module/habanalabs ]]; then
    echo " No device"
    exit 0
fi

mkdir -p tmp
output=tmp/habana-gpu.tmp

# Test that sysinfo finds the cards.  This is also sufficient to test that the GPU SMI library has
# been found and is loaded.

# habana is enabled by default
cargo run -- sysinfo --oldfmt > $output
numcards=$(jq .gpu_cards < $output)
if [[ ! ( $numcards =~ ^[0-9]+$ ) ]]; then
    fail "Bad output from jq: <$numcards>"
fi
if (( numcards == 0 )); then
    fail "Number of cards should be nonzero"
fi

# Run ps once with --load to trigger the collection of gpu utilization data.  This is just a
# smoketest: we're not guaranteed that anything is running on the GPUs and can't really guarantee
# that there is any output.  But on the Habana GPU there is always *some* output.
#
# TODO: This will be cleaner once we have json output.

cargo run -- ps --load --exclude-system-jobs --csv > $output
lines=$(grep -E 'gpuinfo=.*tempc=.*' < $output | wc -l)
if (( lines != 1 )); then
    fail "Number of matching output lines should be 1, got $lines"
fi

echo " OK"
