#!/usr/bin/env bash
#
# Check that the Kafka data sink does its job (without sending anything)

set -e
echo "This test takes about 30s"
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

mkdir -p tmp
outfile=tmp/daemon-kafka-output.txt
logfile=tmp/daemon-kafka-log.txt
if [[ -z $SKIP ]]; then
    rm -rf $outfile $logfile
fi

# If SONARTEST_MOCK_KAFKA is set then the stdout sink is used for both data and select diagnostics.
#
# If it is set to "fail-all-odd-messages" then every odd-numbered message will fail to be enqueued,
# and a message about this is printed on stdout (in addition to appearing in the log).

if [[ -z $SKIP ]]; then
    SONARTEST_MOCK_KAFKA=fail-all-odd-messages cargo run -- daemon daemon-kafka.ini > $outfile 2> $logfile
fi

# The ini produces one record every second but has a 10s sending window and runs the daemon for 30s.
#
# Over a sufficient number of tests there is a reasonably high probability that:
#
#  - a backlog will routinely build up in the sending queue
#  - there will be a backlog that will need to be flushed when the test cutoff is reached
#
# Thus we are testing that:
#
# - messages reach the enqueueing layer as they should
# - odd messages are dropped and even messages are sent
# - the backlog is held after an error
# - the backlog is flushed by stop()
# - the operation of the sending window.
#
# TODO: This seems doable with a little coding:
#
# - test the error callback on the ProducerContext.

prev=-1
num_bad=0
for k in $(jq 'select(has("error"))|.id' < $outfile); do
    num_bad=$((num_bad + 1))
    if (( k - prev != 2 )); then
        echo "Found even key in error output: $k"
        exit 1
    fi
    prev=$((prev + 2))
done

prev=0
num_good=0
for k in $(jq 'select(has("topic"))|.id' < $outfile); do
    num_good=$((num_good + 1))
    if (( k - prev != 2 )); then
        echo "Found odd key in normal output: $k"
        exit 1
    fi
    prev=$((prev + 2))
done

diff=$((num_good - num_bad))
if ((diff < 0)); then
    diff=$((-diff))
fi
if ((diff > 1)); then
    echo "Unlikely good-bad difference $diff"
    exit 1
fi

# Testing that the sending window works: We test that sent messages are batched in several batches
# and that at least some batches have several messages.
#
# Since failing to enqueue a message will stop sending and then re-arm the timer, messages will tend
# to bunch up near the end.  This is fine - we want to test that messages are held long enough.  We
# could make this test more sophisticated by testing bunching not just by sending time, but by the
# time they *could* have been sent in the absence of errors.

prev=0
count=0
batches=0
multibatch=0
for sent in $(jq '.sent' < $outfile); do
    if ((prev < sent)); then
        # New batch
        if ((count > 1)); then
            multibatch=$((multibatch+1))
        fi
        batches=$((batches+1))
        count=1
        prev=$sent
    else
        count=$((count+1))
    fi
done
if ((count > 1)); then
    multibatch=$((multibatch+1))
fi

if ((batches < 4)); then
    echo "No separation of batches detected, batches=$batches"
    exit 1
fi
if ((multibatch < 4)); then
    echo "Insufficient batching detected, multibatch=$multibatch"
    exit 1
fi

echo " Ok"
