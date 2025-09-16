#!/usr/bin/env bash
#
# Test that the --exclude-users switch works.

set -e
numbad=$(cargo run -- ps --exclude-users root,root,root,$LOGNAME | \
    awk "
/,user=root,/ { print }
/,user=$LOGNAME,/ { print }
" | \
    wc -l)
if [[ $numbad -ne 0 ]]; then
    echo "User name filtering did not work"
    exit 1
fi

