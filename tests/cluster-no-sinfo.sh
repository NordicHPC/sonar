#!/usr/bin/env bash
#
# Check that `sonar cluster` produces error output if sinfo is not present.

source sh-helper
assert cargo jq

output=tmp/cluster-no-sinfo.tmp

# Check that sinfo is not available, or we should do nothing

if [[ -n $(command -v sinfo) ]]; then
    echo " sinfo found, skipping"
    exit 0
fi

# The cluster command has only one output format, "new json"

cargo run -- cluster > $output
error=$(jq .errors $output)
if [[ ! ( $error =~ sinfo ) ]]; then
    echo $output
    fail "Expected specific error string, got '$error'"
fi

echo " Ok"
