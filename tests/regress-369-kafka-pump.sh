#!/usr/bin/env bash
#
# Regression test for https://github.com/NordicHPC/sonar/issues/369

source sh-helper

echo "This test takes about 30s"
assert_jq

mkdir -p tmp
outfile=tmp/regress-369-output.txt
logfile=tmp/regress-369-log.txt
if [[ -z $SKIP ]]; then
    rm -f $outfile $logfile
fi
timestamps=tmp/regress-369-timestamps.txt
rm -f $timestamps

# The ini produces one record every second but has a 10s sending window and runs the daemon for 30s.

if [[ -z $SKIP ]]; then
    SONARTEST_MOCK_KAFKA=1 cargo run -- daemon daemon-kafka.ini > $outfile 2> $logfile
fi

# First test that messages are not re-sent: all messages should have distinct time stamps and they
# should be strictly ascending in the output

jq .value.data.attributes.time < $outfile > $timestamps
if ! sort --check=silent $timestamps; then
    fail "Timestamps are not ordered!"
fi
if [[ -n $(uniq --repeated $timestamps) ]]; then
    fail "Timestamps are not unique!"
fi

# Next test that there are no sending windows with zero messages sent
# Possibly the test needs to run much longer and with different settings to *really* test that.

if [[ -n $(grep -E '^Info.*Sending 0 items' $logfile) ]]; then
    fail "Sending zero items!"
fi

# Finally test that a timer is not armed without there being messages to be sent
# Possibly the test needs to run much longer and with different settings to *really* test that.

prev=-100
grep -n -E '^Info.*Sleeping [0-9]+ before sending' $logfile | awk -F: '{ print $1 }' | while read lineno; do
    if (( prev+1 == lineno )); then
        fail "Back-to-back sleeping lines: $prev $lineno"
    fi
    prev=$lineno
done

echo " Ok"
