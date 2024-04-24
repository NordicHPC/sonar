#!/usr/bin/env bash
#
# Primitive test runner.  Keep tests alphabetical.  Note that sysinfo-syntax will require the `jq`
# utility to be installed and will fail if it is not.

set -e
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
