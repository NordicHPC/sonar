#!/usr/bin/env bash
#
# Check that messages that should be sent on startup are actually sent.

source sh-helper
assert cargo jq

echo " This test takes about 10s"

output=$(tmpfile daemon-startup-output)
log=$(tmpfile daemon-startup-log)
inifile=$(tmpfile daemon-startup-ini)

# The startup message will fire (should fire) and then sonar will exit.  If they don't fire, sonar
# will hang because the cadence is very, very slow (8 days).  For that reason, kill it if it is
# still there after a few seconds.
#
# With very low probability, the timed messages will fire at the same time as the startup message,
# ie, it may create an intermittent failure.  It is so rare that it's not worth worrying about it.

for x in sysinfo cluster; do
    cat > $inifile <<EOF
[global]
cluster=x
role=node

[debug]
oneshot=true

[$x]
cadence=192h
EOF

    # The env vars matter only for cluster
    SONARTEST_MOCK_PARTITIONS=testdata/partition_output.txt \
        SONARTEST_MOCK_NODES=testdata/node_output.txt \
        cargo -q run -- daemon $inifile > $output 2> $log &
    pid=$!

    sleep 5
    if kill $pid 2> /dev/null; then
        fail "$x: sonar was still running"
    fi

    result=$(jq -r '.value.data.type' $output)
    if [[ $result != $x ]]; then
        fail "$x: Bad output: $result"
    fi
    if [[ $(wc -l < $log) != 0 ]]; then
        fail "$x: Log is not empty"
    fi
    echo " $x Ok"
done

echo " Ok"
