#!/usr/bin/env bash
#
# Check that `sonar daemon` produces some sane output and can accept some sane input.
# Requirement: the `jq` utility.

set -e
echo " This takes about 15s"
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi

# Run the daemon with the stdio sink and tell it to exit after 10s; the exit is guaranteed to be
# clean.  Capture the output, then make sure the output looks sane.

# The initial echo tries to trick the daemon into exiting early.

rm -f daemon-output.txt
before=$(date +%s)
( echo "exit exit exit" ; sleep 10 ; echo "zappa.hpc.axis-of-eval.org.control.node exit" ) | \
    cargo run -- daemon daemon.ini > daemon-output.txt
after=$(date +%s)

if (( $after - $before < 5 )); then
    echo "Daemon exited too soon"
    exit 1
fi

# jq will read the individual objects in the file and get properties from all, there will typically
# be more than one.  So grab the first line.

topic=$(jq .topic < daemon-output.txt | head -n1)
expect_topic='"zappa.hpc.axis-of-eval.org.sysinfo"'
if [[ $topic != $expect_topic ]]; then
    echo "Bad topic: $topic expected $expect_topic"
    exit 1
fi

key=$(jq .key < daemon-output.txt | head -n1)
expect_key="\"$(hostname)\""
if [[ $key != $expect_key ]]; then
    echo "Bad key: $key expected $expect_key"
    exit 1
fi

client=$(jq .client < daemon-output.txt | head -n1)
expect_client="\"hpc.axis-of-eval.org/$(hostname)\""
if [[ $client != $expect_client ]]; then
    echo "Bad client: $client expected $expect_client"
    exit 1
fi

type=$(jq .value.data.type < daemon-output.txt | head -n1)
expect_type='"sysinfo"'
if [[ $type != $expect_type ]]; then
    echo "Bad type: $type expected $expect_type"
    exit 1
fi

rm -f daemon-output.txt
echo " Ok"
