#!/usr/bin/env bash
#
# Check that `sonar daemon` produces some sane output and can accept some sane input.

source sh-helper
assert cargo jq

echo " This takes about 15s"

output=tmp/daemon-output.txt

# Run the daemon with the stdio sink and tell it to exit after 10s; the exit is guaranteed to be
# clean.  Capture the output, then make sure the output looks sane.

# The initial echo tries to trick the daemon into exiting early.

before=$(date +%s)
( echo "exit exit exit" ; sleep 10 ; echo "zappa.hpc.axis-of-eval.org.control.node exit" ) | \
    cargo run -- daemon daemon.ini > $output
after=$(date +%s)

if (( after - before < 5 )); then
    fail "Daemon exited too soon"
fi

# jq will read the individual objects in the file and get properties from all, there will typically
# be more than one.  So grab the first line.

topic=$(jq .topic $output | head -n1)
expect_topic='"zappa.hpc.axis-of-eval.org.sysinfo"'
if [[ $topic != $expect_topic ]]; then
    fail "Bad topic: $topic expected $expect_topic"
fi

key=$(jq .key $output | head -n1)
expect_key="\"$(hostname)\""
if [[ $key != $expect_key ]]; then
    fail "Bad key: $key expected $expect_key"
fi

client=$(jq .client $output | head -n1)
expect_client="\"hpc.axis-of-eval.org/$(hostname)\""
if [[ $client != $expect_client ]]; then
    fail "Bad client: $client expected $expect_client"
fi

type=$(jq .value.data.type $output | head -n1)
expect_type='"sysinfo"'
if [[ $type != $expect_type ]]; then
    fail "Bad type: $type expected $expect_type"
fi

echo " Ok"
