#!/bin/bash
#
# This will run Kafka in this directory on a custom config file and create some topics if necessary.
# See README.md for more information.

KAFKAROOT=$HOME/lib/kafka_2.13-3.9.0

( cd ../../../util/ssl ; make all )
cp $KAFKAROOT/config/zookeeper.properties .
cp $KAFKAROOT/config/server.properties .
patch -p1 server.properties < server.properties-with-ssl-sasl.diff
echo "STARTING ZOOKEEPER"
$KAFKAROOT/bin/zookeeper-server-start.sh zookeeper.properties &
sleep 5
echo "SLEEPING A LITTLE BEFORE STARTING BROKER"
$KAFKAROOT/bin/kafka-server-start.sh server.properties &
sleep 5
echo "SLEEPING A LITTLE BEFORE CHECKING TOPICS"
lines=$($KAFKAROOT/bin/kafka-topics.sh --bootstrap-server localhost:9099 --list | grep test-server.hpc.uio.no | wc -l)
if (( $lines == 0 )); then
    for service in sample sysinfo cluster job; do
        echo "CREATING TOPIC test-server.hpc.uio.no.$service"
        $KAFKAROOT/bin/kafka-topics.sh --bootstrap-server localhost:9099 --create --topic test-server.hpc.uio.no.$service
    done
fi
echo "SLEEPING A LITTLE MORE"
sleep 3
echo "READY"
