#!/usr/bin/env bash
#
# Check that `sonar sysinfo` produces properly formatted JSON.
# Requirement: the `jq` utility.

set -e
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi

# JSON syntax check

output=$(cargo run -- sysinfo)
jq . <<< $output > /dev/null

echo " JSON ok"

# Superficial CSV check, check that the version number is there

output=$(cargo run -- sysinfo --csv)
if [[ ! ( $output =~ version= ) ]]; then
    echo "CSV missing version number"
    exit 1
fi

echo " CSV ok"

