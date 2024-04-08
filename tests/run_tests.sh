#!/bin/bash
#
# Primitive test runner.  Keep tests alphabetical.  Note that sysinfo-syntax will require the `jq`
# utility to be installed and will fail if it is not.

set -e
for test in command-line hostname interrupt sysinfo-syntax user; do
    echo $test
    ./$test.sh
done
echo "No errors"
