#!/usr/bin/env bash
#
# Test that the --exclude-system-jobs switch works.  System jobs have uid < 1000.

source sh-helper
assert cargo jq

names=$(cargo run -- ps --exclude-system-jobs | jq --raw-output '.data.attributes.jobs[].user' | sort | uniq)
uids=""
for name in $names; do
    uids="$uids $(getent passwd $name | awk -F: '{ print $3 }')"
done
fail=0
for uid in $uids; do
    if (( uid < 1000 )); then
        fail=1
        echo "Unexpected uid $uid"
    fi
done
if (( fail > 0 )); then
    fail "Some system uids reported"
fi
echo " Ok"
exit 0
