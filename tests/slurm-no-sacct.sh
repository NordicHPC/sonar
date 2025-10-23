#!/usr/bin/env bash
#
# Check that `sonar slurm` produces error output if sacct is not present.

source sh-helper
assert cargo jq

# Check that sacct is not available, or we should do nothing

if [[ -n $(command -v sacct) ]]; then
    echo " sacct found, skipping"
    exit 0
fi

output=$(SONARTEST_MOCK_SCONTROL=/dev/null cargo run -- slurm --cluster x --json)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ "sacct" ) ]]; then
    echo $output
    fail "Expected specific error string, got '$error'"
fi

output=$(SONARTEST_MOCK_SCONTROL=/dev/null cargo run -- slurm --csv)
if [[ ! ( $output =~ "error=sacct" ) ]]; then
    fail "Expected specific error string, got '$output'"
fi

echo " Ok"
