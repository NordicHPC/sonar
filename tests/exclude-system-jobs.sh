#!/usr/bin/env bash
#
# Test that the --exclude-system-jobs switch works.  System jobs have uid < 1000.  For each user
# name UNAME, run `getent passwd $UNAME` and then extract the third field from the colon-separated
# list to get the uid, then collect the uids that are < 1000 - these are wrong.

source sh-helper
assert cargo

result=$(cargo run -- ps --exclude-system-jobs --csv | \
             awk '
{
    s=substr($0, index($0, ",user=")+6)
    s=substr(s, 0, index(s, ",")-1)
    uids[s] = 1
}
END {
    s = ""
    for ( uid in uids ) {
        s = s " " uid
    }
    system("getent passwd " s)
}
' | \
             awk -F: '{ if (strtonum($3) < 1000) { print $3 } }')
if [[ -n $result ]]; then
    echo $result
    fail "System jobs filtering did not work"
fi

echo " Ok"
