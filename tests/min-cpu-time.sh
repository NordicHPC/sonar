#!/bin/bash
#
# Test that the --min-cpu-time switch works.

set -e
( cd .. ; cargo build )
numbad=$(../target/debug/sonar ps --min-cpu-time 120 | \
             awk '
{
    s=substr($0, index($0, ",cputime_sec=")+13)
    # this field is frequently last so no guarantee there is a trailing comma
    ix = index(s, ",")
    if (ix > 0)
        s=substr(s, 0, ix-1)
    if (strtonum(s) < 120)
        print($0)
}' | \
             wc -l )
if [[ $numbad -ne 0 ]]; then
    echo "CPU time filtering did not work"
    exit 1
fi
