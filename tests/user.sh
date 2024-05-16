#!/usr/bin/env bash
#
# Check that sonar can look up users.  There will be at least one process for the user: sonar.

set -e
( cd ..; cargo build )
if [[ $(../target/debug/sonar ps | grep ",user=$USER," | wc -l) == 0 ]]; then
    echo "User name lookup fails??"
    exit 1
fi

