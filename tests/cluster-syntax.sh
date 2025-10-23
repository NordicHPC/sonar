#!/usr/bin/env bash
#
# Check that `sonar cluster` produces some sane output.

source sh-helper
assert cargo jq

output=$(tmpfile cluster-syntax)

# Check that sinfo is available, or we should do nothing

if [[ -z $(command -v sinfo) ]]; then
    echo " No sinfo"
    exit 0
fi

# JSON - the only format available

cargo run -- cluster --cluster fox.educloud.no > $output

# Syntax check

jq . $output > /dev/null

# There is always at least an envelope with a version field
version=$(jq .meta.version $output)
if [[ !( $version =~ [0-9]+\.[0-9]+\.[0-9](-.+)? ) ]]; then
    fail "JSON version bad, got $version"
fi

# This is pretty feeble!

echo " JSON ok"
