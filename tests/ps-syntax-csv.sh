#!/usr/bin/env bash
#
# Check that `sonar ps` produces some sane output.

source sh-helper
assert cargo jq

# CSV
#
# At the moment, all we do for CSV is:
#  - check that at least one line is produced
#  - the line starts with `v=` with a sensible version syntax
#  - there is a `host=$HOSTNAME` field
#  - there is a `user=` field with a plausible string (note bug 192, may contain "-")
#  - there is a `cmd=` field
#
# We don't have the infra at the moment to check the CSV output, plus CSV is so flexible that it's
# sort of hard to check it.

output=$(cargo run -- ps --csv)
count=$(wc -l <<< $output)
if (( count <= 0 )); then
    fail "Must have some number of output lines"
fi
l=$(head -n 1 <<< $output)
if [[ !( $l =~ ^v=[0-9]+\.[0-9]+\.[0-9]+(-.+)?, ) ]]; then
    fail "CSV version missing, got $l"
fi
if [[ !( $l =~ ,user=[-a-z0-9_]+, ) ]]; then
    fail "CSV user missing, got $l"
fi
# The command may be quoted so match only the beginning
if [[ !( $l =~ ,\"?cmd= ) ]]; then
    fail "CSV cmd missing, got $l"
fi
if [[ !( $l =~ ,host=$HOSTNAME, ) ]]; then
    fail "CSV host missing, got $l"
fi

echo " Ok: CSV"
