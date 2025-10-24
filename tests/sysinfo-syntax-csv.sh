#!/usr/bin/env bash
#
# Check that `sonar sysinfo` produces properly formatted CSV

source sh-helper
assert cargo jq

# Superficial, check that the version number is there

output=$(cargo run -- sysinfo --csv)
if [[ ! ( $output =~ version= ) ]]; then
    fail "CSV missing version number"
fi

echo " CSV ok"

