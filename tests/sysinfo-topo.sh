#!/usr/bin/env bash
#
# Check that `sonar sysinfo` can produce the topo_text field.

source sh-helper
assert_jq

if [[ -z $(command -v hwloc-ls) ]]; then
    echo "No hwloc-ls; skipping"
    exit 0
fi

output=$(cargo run -- sysinfo --cluster test --json --topo-text-cmd $(command -v hwloc-ls))
field=$(jq .data.attributes.topo_text <<< $output)
if (( $(wc -l <<< $field) != 1 )); then
    fail "Wrong number of output values"
fi

if [[ -n $(command -v base64) ]]; then
    if (( $(base64 -di <<< $field | grep ^Machine | wc -l) != 1 )); then
        fail "Bad output"
    fi
fi

echo " Ok"

