#!/usr/bin/env bash
#
# Run the rollup test in oneshot daemon mode, this will synthesize PIDs for rolled-up jobs.  The
# PIDs should all be different (b/c oneshot) and it should be evident that they are synthesized.

source sh-helper
assert cargo cc

output=$(tmpfile daemon-rollup-out)
inifile=$(tmpfile daemon-rollup-ini)
logfile=$(tmpfile daemon-rollup-log)

make rollup-programs

echo " This takes about 10s"
./rollup 3 &
sleep 3

cat > $inifile <<EOF
[global]
cluster = example.com
role = node

[debug]
verbose = true
oneshot = true

[sample]
cadence = 1s
exclude-system-jobs = true
load = false
rollup = true
EOF

SONARTEST_ROLLUP=1 SONARTEST_ROLLUP_PIDS=30,5 cargo run -- daemon $inifile 2> $logfile > $output

pids=$(jq '.value.data.attributes.jobs[].processes[] | select(.rolledup != null) | .pid' $output)
pid_max=$(cat /proc/sys/kernel/pid_max)
for pid in $pids; do
    if (( pid <= pid_max )); then
        fail "Not synthesized: $pid"
    fi
done

k=0
for pid in $pids; do
    j=0
    for otherpid in $pids; do
        if (( k != j && pid == otherpid )); then
            fail "Not unequal: $pid $otherpid at $k $j"
        fi
        j=$((j + 1))
    done
    k=$((k + 1))
done

echo " Ok"
