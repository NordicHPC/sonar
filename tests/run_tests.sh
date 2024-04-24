#!/usr/bin/env bash
#
# Primitive test runner.

set -e

# sysinfo-syntax.sh requires jq
# check whether jq is available and exit if not
if ! command -v jq &> /dev/null; then
    echo "ERROR: jq is required for sysinfo-syntax.sh"
    exit 1
fi

# keep tests alphabetical
# later we could just iterate over all scripts that end with .sh
# and are not this script
for test in command-line \
                exclude-commands \
                exclude-system-jobs \
                exclude-users \
                hostname \
                interrupt \
                lockfile \
                min-cpu-time \
                ps-syntax \
                sysinfo-syntax \
                user \
            ; do
    echo $test
    ./$test.sh
done

echo "No errors"
