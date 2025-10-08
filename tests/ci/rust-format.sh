#!/bin/bash
#
# Test that the Rust code is formatted properly.

output=$(cargo fmt --check --message-format short)
if [[ -n $output ]]; then
    echo "FORMATTING FAILURE!"
    echo "The following files are not properly formatted (cargo fmt):"
    echo "$output"
    exit 1
fi
