#!/usr/bin/env bash
#
# Check that `sonar slurm` produces error output if scontrol is not present.

source sh-helper
assert cargo jq

# Check that scontrol is not available, or we should do nothing

if [[ -n $(command -v scontrol) ]]; then
    echo " scontrol found, skipping"
    exit 0
fi

output=$(SONARTEST_MOCK_SACCT=/dev/null cargo run -- slurm)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ scontrol ) ]]; then
    echo $output
    fail "Expected specific JSON error string, got '$error'"
fi

# The scontrol failure is not surfaced for the old CSV output format

echo " Ok"
