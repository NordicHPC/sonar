#!/usr/bin/env bash
#
# Check that `sonar slurm` produces some sane output.

source sh-helper
assert cargo jq

# Check that sacct is available, or we should do nothing

if [[ -z $(command -v sacct) ]]; then
    echo " No sacct"
    exit 0
fi

# JSON

output=$(cargo run -- slurm --cluster x --json)

# Syntax check

jq . <<< $output > /dev/null

# There is always at least an envelope with a version field
version=$(jq .meta.version <<< $output)
if [[ !( $version =~ [0-9]+\.[0-9]+\.[0-9](-.+)? ) ]]; then
    fail "JSON version bad, got $version"
fi

echo " Ok: JSON"
