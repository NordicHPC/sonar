#!/usr/bin/env bash
#
# Check that `sonar slurm` produces some sane output.

source sh-helper
assert_jq

# Check that sacct is available, or we should do nothing

if [[ -z $(command -v sacct) ]]; then
    echo " No sacct"
    exit 0
fi

# CSV
#
# There's no guarantee that there is a record.

output=$(cargo run -- slurm --csv)
if [[ -z $output ]]; then
    echo " Ok: No output"
    exit 0
fi

# If there is output it should at least have a version field
l=$(head -n 1 <<< $output)
if [[ !( $l =~ ^v=[0-9]+\.[0-9]+\.[0-9](-.+)?, ) ]]; then
    fail "CSV version missing, got $l"
fi

echo " Ok: CSV"

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
