#!/usr/bin/env bash
#
# Check that `sonar slurm` produces error output if sacct is not present.

set -e
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

# Check that sacct is not available, or we should do nothing

if [[ -n $(command -v sacct) ]]; then
    echo " sacct found, skipping"
    exit 0
fi

output=$(cargo run -- slurm --cluster x --json)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ "sacct" ) ]]; then
    echo $output
    echo "Expected specific error string, got '$error'"
    exit 1
fi

output=$(cargo run -- slurm --csv)
if [[ ! ( $output =~ "error=sacct" ) ]]; then
    echo "Expected specific error string, got '$output'"
    exit 1
fi

echo " Ok"
