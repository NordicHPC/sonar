#!/bin/bash
#
# Note, this should not be run by `run_tests.sh` since it is normal for code to be unformatted
# during development.

output=$(cargo fmt --check --message-format short)
if [[ $output != "" ]]; then
    echo "FORMATTING FAILURE!"
    echo "The following files are not properly formatted (cargo fmt):"
    echo "$output"
    exit 1
fi
