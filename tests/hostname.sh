#!/usr/bin/env bash
#
# Check that sonar reports the correct hostname

source sh-helper
assert cargo jq

for opt in "" --hostname-only; do
    if [[ -z $opt ]]; then
        expect=$(hostname)
    else
        expect=$(hostname | grep -o -E '^[a-zA-Z0-9-]+')
    fi
    for cmd in ps sysinfo; do
        node=$(cargo run -- $cmd $opt | jq --raw-output '.data.attributes.node')
        if [[ $node != $expect ]]; then
            fail "$cmd - Wrong hostname?  Got $node expected $expect"
        fi
    done
done

# The normal case for node lists is that there are no dots, but test at least that there are none if
# we ask for there to be none.

if [[ -n $(command -v sinfo) ]]; then

    # The 'partitions' arrays maps partition names to node sets
    if cargo run -- cluster --hostname-only | jq '.data.attributes.partitions[].nodes[]' | grep -F '.'; then
        fail "cluster produced output with dots in partitions"
    fi

    # The 'nodes' array is really the 'states' array, it maps node sets to state sets that the nodes share
    if cargo run -- cluster --hostname-only | jq '.data.attributes.nodes[].names[]' | grep -F '.'; then
        fail "cluster produced output with dots in nodes"
    fi

    # Not all jobs have nodes, notably, jobs that were cancelled before resources were allocated
    if cargo run -- jobs | jq -r '.data.attributes.slurm_jobs[].nodes | select(. != null) | .[]' | grep -F '.'; then
        fail "jobs produced output with dots"
    fi

fi

echo " Ok"
