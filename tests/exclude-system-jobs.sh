#!/usr/bin/env bash
#
# Test that the --exclude-system-jobs switch works.  System jobs have uid < 1000 and the
# InContainer flag should not be set for any of their processes.
#
# Sometimes it's useful to run this test at the same time as this dockerized workload:
#
#   make pincpu
#   docker run -d -v .:/sonar:z --rm -it ubuntu /sonar/pincpu 30
#
# In that case, the output should show that at least one dockerized job was found.

source sh-helper
assert cargo jq go

cargo run -- ps --exclude-system-jobs | \
    jq -c '.data.attributes.jobs[] | [.user, .processes[].in_container]' | \
    go run exclude-system-jobs.go

echo " Ok"
exit 0
