#!/bin/bash
#
# Check that `sonar ps` produces some sane output.
#
# At the moment, all we do is:
#  - check that at least one line is produced
#  - the line starts with `v=` with a sensible version syntax
#  - there is a `host=$HOSTNAME` field
#  - there is a `user=` field with a plausible string
#  - there is a `cmd=` field
#
# We don't have the infra at the moment to check the CSV output (cf sysinfo-syntax.sh where we use
# the jq utility), plus CSV is so flexible that it's sort of hard to check it.

set -e
( cd .. ; cargo build )
output=$(../target/debug/sonar ps)
count=$(wc -l <<< $output)
if [[ $count -le 0 ]]; then
    echo "Must have some number of output lines"
    exit 1
fi
l=$(head -n 1 <<< $output)
if [[ !( $l =~ ^v=[0-9]+\.[0-9]+\.[0-9], ) ]]; then
    echo "Version missing"
    exit 1
fi
if [[ !( $l =~ ,user=[a-z0-9]+, ) ]]; then
    echo "User missing"
    exit 1
fi
# The command may be quoted so match only the beginning
if [[ !( $l =~ ,\"?cmd= ) ]]; then
    echo "Cmd missing"
    exit 1
fi
if [[ !( $l =~ ,host=$HOSTNAME, ) ]]; then
    echo "Host missing"
    exit 1
fi
