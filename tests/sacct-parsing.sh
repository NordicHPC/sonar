#!/usr/bin/env bash
#
# Check that `sonar slurm` produces correct output from a known input.

source sh-helper
assert cargo jq

sonar_output=$(tmpfile sacct-parsing-sacct-output)
jobs1=$(tmpfile sacct-parsing-jobs1)
jobs2=$(tmpfile sacct-parsing-jobs2)

# This is pretty harsh: we require bitwise-identical output.  The assumption is that we have
# hand-checked the output in testdata.  An alternative is to descend into the produced data here
# with jq and make sure specific values are as expected.  But it amounts to the same thing.

# The TZ must be set because the sacct-produced data are timezone-less but the sonar-generated
# expected result is not. CET-2 is UTC+2, corresponding to those data.

TZ=CET-2 \
    SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    SONARTEST_MOCK_SCONTROL=/dev/null \
    cargo run -- slurm --deluge --json --cluster fox.educloud.no \
    > $sonar_output
# Strip the envelope because it contains a timestamp for when the data were generated.
jq .data.attributes.slurm_jobs < testdata/sonar_sacct_output.txt > $jobs1
jq .data.attributes.slurm_jobs < $sonar_output > $jobs2
if ! cmp $jobs1 $jobs2; then
    diff $jobs1 $jobs2
    fail "Sonar output differs!"
fi

echo " Ok"
