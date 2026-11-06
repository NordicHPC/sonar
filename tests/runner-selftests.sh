#!/usr/bin/env bash
#
# Check that the root runner compiles and passes selftests.

source sh-helper
assert cmake cc valgrind

cd ../runner
make check
