#!/usr/bin/env bash
#
# Check that `sonar cluster` produces error output if sinfo is not present.

source sh-helper

assert_jq

# Check that sinfo is not available, or we should do nothing

if [[ -n $(command -v sinfo) ]]; then
    echo " sinfo found, skipping"
    exit 0
fi

output=$(cargo run -- cluster --cluster x --json)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ "sinfo" ) ]]; then
    echo $output
    fail "Expected specific error string, got '$error'"
fi

# Default output is also "new json"

output=$(cargo run -- cluster --cluster x)
error=$(jq .errors <<< $output)
if [[ ! ( $error =~ "sinfo" ) ]]; then
    echo $output
    fail "Expected specific error string, got '$error'"
fi

echo " Ok"
