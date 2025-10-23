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
