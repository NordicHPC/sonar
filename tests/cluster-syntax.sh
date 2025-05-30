#!/usr/bin/env bash
#
# Check that `sonar cluster` produces some sane output.
# Requirement: the `jq` utility.

set -e
( cd .. ; cargo build )
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi

# Check that sinfo is available, or we should do nothing

if [[ $(command -v sinfo) == "" ]]; then
    echo " No sinfo"
    exit 0
fi

# JSON - the only format available

output=$(../target/debug/sonar cluster --cluster x --json)

# Syntax check

jq . <<< $output > /dev/null

# There is always at least an envelope with a version field
version=$(jq .meta.version <<< $output)
if [[ !( $version =~ [0-9]+\.[0-9]+\.[0-9](-.+)? ) ]]; then
    echo "JSON version bad, got $version"
    exit 1
fi

# This is pretty feeble!

echo " JSON ok"
