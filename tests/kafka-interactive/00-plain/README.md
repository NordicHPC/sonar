# Manual kafka testing

Following https://kafka.apache.org/42/getting-started/quickstart/, download Kafka and then create a
broker (I used the docker setup on that page).

Then in the Kafka dir create a topic:
```
bin/kafka-topics.sh --create --topic cluster.sysinfo --bootstrap-server localhost:9092
```

## Direct connection from Sonar to Kafka

In this dir in one window:
```
cargo run -- daemon sonar-direct.cfg
```

Then in another window:
```
bin/kafka-console-consumer.sh --topic cluster.sysinfo --from-beginning --bootstrap-server localhost:9092
```

Observe that we see the messages that have been delivered.

## Indirect connection from Sonar via Kafka-proxy to Kafka

In this dir in one window:
```
( cd ../../../util/kafka-proxy ; go build )
../../../kafka-proxy/kprox -d kprox.ini
```

In this dir in another window:
```
cargo run -- daemon sonar-kprox.cfg
```

Then in a third window:
```
bin/kafka-console-consumer.sh --topic cluster.sysinfo --from-beginning --bootstrap-server localhost:9092
```

Observe that we see the messages that have been delivered (beware messages lingering from the
previous test - observe the timestamps).
