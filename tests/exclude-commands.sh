#!/usr/bin/env bash
#
# Test that the --exclude-commands switch works.

set -e
( cd .. ; cargo build )
numbad=$(../target/debug/sonar ps --exclude-commands bash,sh,zsh,csh,ksh,tcsh,kworker | \
    awk "
/,cmd=kworker/ { print }
/,cmd=(ba|z|c|k|tc|)sh/ { print }
" | \
    wc -l)
if [[ $numbad -ne 0 ]]; then
    echo "Command filtering did not work"
    exit 1
fi

