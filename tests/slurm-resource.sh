#!/usr/bin/env bash
#
# Check that `sonar slurm` output picks up resource information.  It should prefer output from
# scontrol over output from sacct.
#
# We have two test files, one for sacct output with three records and one for scontrol output with
# another three.  The former has job IDs A, B, and C with resource info in A and C.  The latter has
# job IDs A, B, and D with resource info in A and D.  The resource info in A differs among the two.
#
# The output should be three jobs, A, B, and C with resource info in A from scontrol, no resource
# info in B, and the resource info in C from sacct.

source sh-helper
assert cargo jq

outfile=$(tmpfile slurm-resource)
expected=$(tmpfile slurm-resource-expected)
computed=$(tmpfile slurm-resource-computed)

TZ=/usr/share/zoneinfo/Europe/Oslo \
    SONARTEST_MOCK_SACCT=testdata/sacct_resource.txt \
    SONARTEST_MOCK_SCONTROL=testdata/scontrol_resource.txt \
    cargo run -- slurm --deluge > $outfile

cat > $expected <<EOF
1879824
"cpu=8,mem=120G,node=1,billing=99,gres/gpu=1"
1903070
null
1902893
"billing=78,cpu=48,gres/gpu:a100_80=4,gres/gpu=4,mem=384G,node=1"
EOF

jq '.data.attributes.slurm_jobs[] | .job_id, .gres_detail' $outfile > $computed
cmp $expected $computed

echo " Ok"


