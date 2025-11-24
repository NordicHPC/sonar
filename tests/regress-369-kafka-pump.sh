#!/usr/bin/env bash
#
# Regression test for https://github.com/NordicHPC/sonar/issues/369

source sh-helper
assert cargo jq

echo "This test takes about 30s"

outfile=$(tmpfile regress-369-output)
logfile=$(tmpfile regress-369-log)
timestamps=$(tmpfile regress-369-timestamps)
inifile=$(tmpfile regress-369-ini)

cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=node

[debug]
verbose = true
time-limit = 30s

[kafka]
broker-address = no.such.host:101010
sending-window = 10s

[sysinfo]
cadence=1s
EOF

# The ini produces one record every second but has a 10s sending window and runs the daemon for 30s.

if [[ -z $SKIP ]]; then
    SONARTEST_MOCK_KAFKA=1 cargo run -- daemon $inifile > $outfile 2> $logfile
fi

# First test that messages are not re-sent: all messages should have distinct time stamps and they
# should be strictly ascending in the output

jq .value.data.attributes.time < $outfile > $timestamps
if ! sort --check=silent $timestamps; then
    fail "Timestamps are not ordered!"
fi
if [[ -n $(uniq --repeated $timestamps) ]]; then
    cat $timestamps
    fail "Timestamps are not unique!"
fi

# Next test that there are no sending windows with zero messages sent
# Possibly the test needs to run much longer and with different settings to *really* test that.

if grep -q -E '^Info.*Sending 0 items' $logfile; then
    fail "Sending zero items at least once!"
fi

# Finally test that a timer is not armed without there being messages to be sent
# Possibly the test needs to run much longer and with different settings to *really* test that.

prev=-100
grep -n -E 'DEBUG.*Sleeping [0-9]+ before sending' $logfile | \
    awk -F: '{ print $1 }' | \
    while read lineno; do
        if (( prev+1 == lineno )); then
            fail "Back-to-back sleeping lines: $prev $lineno"
        fi
        prev=$lineno
    done

echo " Ok"
