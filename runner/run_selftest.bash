#!/usr/bin/env bash

# Selftest depends on these
export XZAPPAY=xzappayval
export ZAPPA_PREFIX=zappaprefixval
export SUFFIX_ZAPPA=zappasuffixval
export ZAPPA=zappaval

binary=./selftest
if [[ ! -x $binary ]]; then
    binary=build/selftest
fi
if [[ ! -x $binary ]]; then
    echo "No selftest binary"
    exit 0
fi
valgrind --leak-check=full --trace-children=yes --error-exitcode=1 $binary
