#!/usr/bin/env bash
#
# Check that `sonar slurm` output picks up resource information.  It should prefer output from
# scontrol over output from sacct.
#
# We have two test files, one for sacct output with three records and one for scontrol output with
# another three.  The former has job IDs A, B, and C with resource info in A and C.  The latter has
# job IDs A, B, and D with resource info in A and D.  The resource info in A differs among the two.
#
# The output should be three jobs, A, B, and C with resource info in A from scontrol and sacct, no
# resource info in B, and the resource info in C from sacct.
#
# We do this twice, once with the scontrol files identifying the resource as ReqTRES and once
# identifying it as TRES, as in older SLURM versions.

source sh-helper
assert cargo jq

outfile=tmp/slurm-resource.tmp
expected=tmp/slurm-resource-expected.tmp
computed=tmp/slurm-resource-computed.tmp

cat > $expected <<EOF
1879824
"cpu=8,mem=120G,node=1,billing=99,gres/gpu=1"
"billing=32,cpu=32,gres/gpu:a100=1,gres/gpu=1,mem=128G,node=1"
1903070
null
null
1902893
null
"billing=78,cpu=48,gres/gpu:a100_80=4,gres/gpu=4,mem=384G,node=1"
EOF

# Timezone is set to UTC+2 to be consistent with current test data, although for this
# test it doesn't matter.

echo " First run"
TZ=CET-2 \
    SONARTEST_MOCK_SACCT=testdata/sacct_resource.txt \
    SONARTEST_MOCK_SCONTROL=testdata/scontrol_resource1.txt \
    cargo run -- slurm --deluge > $outfile

jq '.data.attributes.slurm_jobs[] | .job_id, .requested_resources, .allocated_resources' $outfile > $computed
cmp $expected $computed

echo " Second run"
TZ=CET-2 \
    SONARTEST_MOCK_SACCT=testdata/sacct_resource.txt \
    SONARTEST_MOCK_SCONTROL=testdata/scontrol_resource2.txt \
    cargo run -- slurm --deluge > $outfile

jq '.data.attributes.slurm_jobs[] | .job_id, .requested_resources, .allocated_resources' $outfile > $computed
cmp $expected $computed

echo " Ok"


