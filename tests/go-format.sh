#!/bin/bash
#
# Note, this should not be run by `run_tests.sh` since it is normal for code to be unformatted
# during development.

output=$(gofmt -l ../util)
if [[ $output != "" ]]; then
    echo "FORMATTING FAILURE!"
    echo "The following files are not properly formatted (go fmt):"
    echo "$output"
    exit 1
fi
