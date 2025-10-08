#!/bin/bash
#
# Test that the Go code is formatted properly.

output=$(gofmt -l ../../util)
if [[ -n $output ]]; then
    echo "FORMATTING FAILURE!"
    echo "The following files are not properly formatted (go fmt):"
    echo "$output"
    exit 1
fi
