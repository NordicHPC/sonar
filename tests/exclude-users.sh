#!/usr/bin/env bash
#
# Test that the --exclude-users switch works.

source sh-helper
assert cargo

result=$(cargo run -- ps --exclude-users root,root,root,$LOGNAME | \
    awk "
/,user=root,/ { print }
/,user=$LOGNAME,/ { print }
")
if [[ -n $result ]]; then
    echo $result
    fail "User name filtering did not work"
fi

echo " Ok"
