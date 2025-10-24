#!/usr/bin/env bash
#
# Check that `sonar sysinfo` produces properly formatted JSON.

source sh-helper
assert cargo jq

output=$(cargo run -- sysinfo)
jq . <<< $output > /dev/null

echo " JSON ok"
