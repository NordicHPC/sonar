#!/usr/bin/env bash
#
# Regression test for https://github.com/NordicHPC/sonar/issues/369

set -e
echo "This test takes about 30s"
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

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
    echo "Timestamps are not ordered!"
    exit 1
fi
if [[ -n $(uniq --repeated $timestamps) ]]; then
    echo "Timestamps are not unique!"
    exit 1
fi

# Next test that there are no sending windows with zero messages sent
# Possibly the test needs to run much longer and with different settings to *really* test that.

if [[ -n $(grep -E '^Info.*Sending 0 items' $logfile) ]]; then
    echo "Sending zero items!"
    exit 1
fi

# Finally test that a timer is not armed without there being messages to be sent
# Possibly the test needs to run much longer and with different settings to *really* test that.

prev=-100
grep -n -E '^Info.*Sleeping [0-9]+ before sending' $logfile | awk -F: '{ print $1 }' | while read lineno; do
    if (( prev+1 == lineno )); then
        echo "Back-to-back sleeping lines: $prev $lineno"
        exit 1
    fi
    prev=$lineno
done

echo " OK"
exit 0
