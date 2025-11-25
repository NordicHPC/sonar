#!/usr/bin/env bash
#
# Check that `sonar slurm` output is batched correctly in several ways:
#
# - by comparing the non-batched output with the catenated batched output
# - by checking that no batch is larger than requested
# - by counting output records in the daemon data sink via metadata and comparing it
#   to the expected number of batches

source sh-helper
assert cargo jq

# Currently there are 301=43*7 input records, any batch size not those factors is OK.
batch=17

outfile1=$(tmpfile sacct-batching1)
outfile2=$(tmpfile sacct-batching2)
inifile=$(tmpfile sacct-batching-ini)
logfile=$(tmpfile sacct-batching-log)

SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    SONARTEST_MOCK_SCONTROL=/dev/null \
    cargo run -- slurm --deluge \
    | jq '.data.attributes.slurm_jobs[]|[.job_id,.job_name,.job_state]' > $outfile1

SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    SONARTEST_MOCK_SCONTROL=/dev/null \
    cargo run -- slurm --deluge --batch-size $batch \
    | jq '.data.attributes.slurm_jobs[]|[.job_id,.job_name,.job_state]' > $outfile2

cmp $outfile1 $outfile2

expected_records=0
for n in $(SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
               SONARTEST_MOCK_SCONTROL=/dev/null \
               cargo run -- slurm --deluge --batch-size $batch \
               | jq '.data.attributes.slurm_jobs|length'); do
    expected_records=$((expected_records+1))
    if ((n > batch)); then
        fail "Batch size is off: $n"
    fi
done

# Test that the batches are broken into actual separate messages in the data sink and not just
# catenated in the output; we can't tell from the above.
#
# Use the daemon's Kafka output path with SONARTEST_MOCK_KAFKA to force output to stdout and
# metadata to stderr.  Force the input with SONARTEST_MOCK_{SACCT,SCONTROL} and force the daemon to
# quit after one run with a debug setting.
#
# Note that any sending-window is OK here.  If we set it to 1s, say, then the timer will fire and
# re-arm repeatedly because the timer fires before all the data can be sent, and the output will
# reflect this.  But the data should all still be sent.  If we set the window to 10s instead then
# there will typically be many fewer firings, because all the messages will be queued up before the
# first firing (unless sonar randomly picks a short window for the first fire).

cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=master

[debug]
verbose = true
oneshot = true

[kafka]
broker-address = no.such.host:0000
sending-window = 10s

[jobs]
cadence=1s
batch-size = 17
EOF

SONARTEST_MOCK_KAFKA=1 \
    SONARTEST_MOCK_SACCT=testdata/sacct_output.txt \
    SONARTEST_MOCK_SCONTROL=/dev/null \
    cargo run -- daemon $inifile > /dev/null 2> $logfile

actual_records=$(grep 'DEBUG.*Sending to topic: ' $logfile | wc -l)
if ((actual_records != expected_records)); then
    cat $logfile
    fail "Wrong number of records sent, expected $expected_records got $actual_records"
fi

echo " Ok"
