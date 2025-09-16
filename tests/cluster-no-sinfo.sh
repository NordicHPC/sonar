#!/usr/bin/env bash
#
# Check that `sonar cluster` produces error output if sinfo is not present.
# Requirement: the `jq` utility.

set -e
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi

# Check that sacct is not available, or we should do nothing

if [[ $(command -v sinfo) != "" ]]; then
    echo " sinfo found, skipping"
    exit 0
fi

output=$(cargo run -- cluster --cluster x --json)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ "sinfo" ) ]]; then
    echo $output
    echo "Expected specific error string, got '$error'"
    exit 1
fi

# Default output is also "new json"

output=$(cargo run -- cluster --cluster x)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ "sinfo" ) ]]; then
    echo $output
    echo "Expected specific error string, got '$error'"
    exit 1
fi

echo " Ok"
