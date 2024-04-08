#!/bin/bash
#
# Primitive test runner.  Keep tests alphabetical.  Note that sysinfo-syntax will require the `jq`
# utility to be installed and will fail if it is not.

./command-line.sh
./hostname.sh
./sysinfo-syntax.sh
./user.sh
