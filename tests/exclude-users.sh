#!/usr/bin/env bash
#
# Test that the --exclude-users switch works.

source sh-helper

numbad=$(cargo run -- ps --exclude-users root,root,root,$LOGNAME | \
    awk "
/,user=root,/ { print }
/,user=$LOGNAME,/ { print }
" | \
    wc -l)
if (( numbad != 0 )); then
    fail "User name filtering did not work"
fi

echo " Ok"
