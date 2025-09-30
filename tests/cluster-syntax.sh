#!/usr/bin/env bash
#
# Check that `sonar cluster` produces some sane output.

set -e
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

# Check that sinfo is available, or we should do nothing

if [[ -z $(command -v sinfo) ]]; then
    echo " No sinfo"
    exit 0
fi

# JSON - the only format available

output=$(cargo run -- cluster --cluster fox.educloud.no --json)

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
