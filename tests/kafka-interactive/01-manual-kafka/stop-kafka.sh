#!/bin/bash
#
# This will cleanly stop a Kafka that was started by run-kafka.sh.  See README.md for more
# information.

KAFKAROOT=$HOME/lib/kafka_2.13-3.9.0

$KAFKAROOT/bin/kafka-server-stop.sh --bootstrap-server localhost:9099
$KAFKAROOT/bin/zookeeper-server-stop.sh
