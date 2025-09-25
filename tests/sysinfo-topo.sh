#!/usr/bin/env bash
#
# Check that `sonar sysinfo` can produce the topo_text field.
# Requirement: the `jq` utility.

set -e
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

if [[ -z $(command -v hwloc-ls) ]]; then
    echo "No hwloc-ls; skipping"
    exit 0
fi

output=$(cargo run -- sysinfo --cluster test --json --topo-text-cmd $(command -v hwloc-ls))
field=$(jq .data.attributes.topo_text <<< $output)
if [[ $(wc -l <<< $field) != 1 ]]; then
    echo "Wrong number of output values"
    exit 1
fi

if [[ -n $(command -v base64) ]]; then
    if [[ $(base64 -di <<< $field | grep ^Machine | wc -l) != 1 ]]; then
        echo "Bad output"
        exit 1
    fi
fi

echo " Ok"

