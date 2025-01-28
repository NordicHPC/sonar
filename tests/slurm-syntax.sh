#!/usr/bin/env bash
#
# Check that `sonar slurm` produces some sane output.
# Requirement: the `jq` utility.

set -e
( cd .. ; cargo build )
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi

# Check that sacct is available, or we should do nothing

if [[ $(command -v sacct) == "" ]]; then
    echo "No sacct"
    exit 0
fi

# CSV
#
# There's no guarantee that there is a record.

output=$(../target/debug/sonar slurm)
if [[ $output == "" ]]; then
    echo "No output"
    exit 0
fi

# If there is output it should at least have a version field
l=$(head -n 1 <<< $output)
if [[ !( $l =~ ^v=[0-9]+\.[0-9]+\.[0-9](-.+)?, ) ]]; then
    echo "CSV version missing, got $l"
    exit 1
fi

echo "CSV ok"

# JSON

output=$(../target/debug/sonar slurm --json)

# Syntax check

jq . <<< $output > /dev/null

# There is always at least an envelope with a version field
version=$(jq .v <<< $output)
if [[ !( $version =~ [0-9]+\.[0-9]+\.[0-9](-.+)? ) ]]; then
    echo "JSON version bad, got $version"
    exit 1
fi

echo "JSON ok"
