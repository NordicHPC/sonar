#!/bin/bash
#
# Test that the C code is formatted properly.

errors=0
for d in ../../runner ../../gpuapi; do
    output=$(cd $d ; clang-format --dry-run --style=file:../clang-format.txt *.c *.h 2>&1)
    if [[ -n $output ]]; then
        echo "C FORMATTING FAILURES IN $d!"
        echo "$output"
        errors=1
    fi
done
if ((errors != 0)); then
    exit 1
fi
