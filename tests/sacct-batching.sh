#!/usr/bin/env bash
#
# Check that `sonar slurm` output is batched correctly by comparing the non-batched output with the
# catenated batched output, and by checking that no batch is larger than requested.

source sh-helper
assert cargo jq

# Currently there are 301=43*7 input records, any batch size not those factors is OK.
batch=17

outfile1=$(tmpfile sacct-batching1)
outfile2=$(tmpfile sacct-batching2)

SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    SONARTEST_MOCK_SCONTROL=/dev/null \
    cargo run -- slurm --deluge \
    | jq '.data.attributes.slurm_jobs[]|[.job_id,.job_name,.job_state]' > $outfile1

SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    SONARTEST_MOCK_SCONTROL=/dev/null \
    cargo run -- slurm --deluge --batch-size $batch \
    | jq '.data.attributes.slurm_jobs[]|[.job_id,.job_name,.job_state]' > $outfile2

cmp $outfile1 $outfile2

for n in $(SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
               SONARTEST_MOCK_SCONTROL=/dev/null \
               cargo run -- slurm --deluge --batch-size $batch \
               | jq '.data.attributes.slurm_jobs|length'); do
    if ((n > batch)); then
        fail "Batch size is off: $n"
    fi
done

echo " Ok"
