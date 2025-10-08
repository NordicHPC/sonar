#!/usr/bin/env bash
#
# Test that the --min-cpu-time switch works.

source sh-helper

numbad=$(cargo run -- ps --min-cpu-time 5 | \
             awk '
{
    s=substr($0, index($0, ",cputime_sec=")+13)
    # this field is frequently last so no guarantee there is a trailing comma
    ix = index(s, ",")
    if (ix > 0)
        s=substr(s, 0, ix-1)
    if (strtonum(s) < 5)
        print($0)
}' | \
             wc -l )
if (( numbad != 0 )); then
    fail "CPU time filtering did not work"
fi

echo " Ok"
