#!/bin/bash
#
# Test that the --exclude-system-jobs switch works.  System jobs have uid < 1000.  For each user
# name UNAME, run `getent passwd $UNAME` and then extract the third field from the colon-separated
# list to get the uid, then collect the uids that are < 1000 - these are wrong.

set -e
( cd .. ; cargo build )
numbad=$(../target/debug/sonar ps --exclude-system-jobs | \
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
             awk -F: '{ if (strtonum($3) < 1000) { print $3 } }' | \
             wc -l )
if [[ $numbad -ne 0 ]]; then
    echo $numbad
    echo "System jobs filtering did not work"
    exit 1
fi

