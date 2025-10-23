#!/usr/bin/env bash
#
# Check that sonar reports the correct hostname

source sh-helper
assert cargo jq

node=$(../target/debug/sonar ps | jq --raw-output '.data.attributes.node')
if [[ $node != $(hostname) ]]; then
    fail "Wrong hostname?  Got $node"
fi

echo " Ok"
