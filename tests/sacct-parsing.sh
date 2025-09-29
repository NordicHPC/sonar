#!/usr/bin/env bash
#
# Check that `sonar slurm` produces correct output from a known input.
# Requirement: the `jq` utility.

set -e
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

sonar_output=sacct-parsing-sacct-output.tmp
jobs1=sacct-parsing-jobs1.tmp
jobs2=sacct-parsing-jobs2.tmp

# This is pretty harsh: we require bitwise-identical output.  The assumption is that we have
# hand-checked the output in testdata.  An alternative is to descend into the produced data here
# with jq and make sure specific values are as expected.  But it amounts to the same thing.

# The TZ must be set because the sacct-produced data are timezone-less but the sonar-generated
# expected result is not.

rm -f $sonar_output $jobs1 $jobs2
TZ=/usr/share/zoneinfo/Europe/Oslo \
    SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    cargo run -- slurm --deluge --json --cluster fox.educloud.no \
    > $sonar_output
# Strip the envelope because it contains a timestamp for when the data were generated.
jq .data.attributes.slurm_jobs < testdata/sonar_sacct_output.txt > $jobs1
jq .data.attributes.slurm_jobs < $sonar_output > $jobs2
if ! cmp $jobs1 $jobs2; then
    echo "Sonar output differs"
    diff $jobs1 $jobs2
    exit 1
fi

rm -f $sonar_output $jobs1 $jobs2
echo " Ok"
