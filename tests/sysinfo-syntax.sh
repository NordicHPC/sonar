#!/usr/bin/env bash
#
# Check that `sonar sysinfo` produces properly formatted JSON.
# Requirement: the `jq` utility.

set -e
( cd .. ; cargo build )
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi
output=$(../target/debug/sonar sysinfo)
jq . <<< $output > /dev/null
