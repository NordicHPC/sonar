#!/usr/bin/env bash
#
# Check that `sonar sysinfo` produces properly formatted JSON.

source sh-helper
assert_jq

# JSON syntax check

output=$(cargo run -- sysinfo)
jq . <<< $output > /dev/null

echo " JSON ok"

# Superficial CSV check, check that the version number is there

output=$(cargo run -- sysinfo --csv)
if [[ ! ( $output =~ version= ) ]]; then
    fail "CSV missing version number"
fi

echo " CSV ok"

