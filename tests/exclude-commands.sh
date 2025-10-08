#!/usr/bin/env bash
#
# Test that the --exclude-commands switch works.

source sh-helper

numbad=$(cargo run -- ps --exclude-commands bash,sh,zsh,csh,ksh,tcsh,kworker | \
    awk "
/,cmd=kworker/ { print }
/,cmd=(ba|z|c|k|tc|)sh/ { print }
" | \
    wc -l)
if (( numbad != 0 )); then
    fail "Command filtering did not work"
fi

echo " Ok"
