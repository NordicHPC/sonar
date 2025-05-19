# Running Kafka and Kafka Admin UI using docker-compose

This test tests sonar in combination with a docker-based startup of kafka and admin UI - all using SSL and SASL.

1. Ensure that `docker` and `docker compose` are available on your system.
2. Then start the kafka broker and the kafka administration ui via

```
   $> ./run-kafka-docker.sh
```

3. Check that you can access http://localhost:10090/ui/clusters/test-cluster and you see and green dot behind the cluster name 'test-cluster'.
   The green dot means, that the UI successfully connected to the broker. You can also verfiy that one broker host 'kafka-broker' exists.
   No topics can be seen at this stage.


4. Start sonar and wait for it to report on the messages send:

```
   $> ./run-sonar.sh
   Compiling sonar v0.13.0-devel (/workspace/sonar)
    Finished dev [unoptimized + debuginfo] target(s) in 0.92s
   Info: Initialization succeeded
   Info: Sysinfo
   Info: Waiting for stuff to send
   Info: Sleeping 13 before sending
   Info: Sysinfo
   Info: Sending to topic: test-cluster.hpc.uio.no.sysinfo with id 1
   Info: Sending to topic: test-cluster.hpc.uio.no.sysinfo with id 2
   Info: Waiting for stuff to send
   Info: Sent #1 successfully
   Info: Sent #2 successfully
   Info: Sysinfo
   Info: Sleeping 45 before sending
   Info: Sysinfo
```

5. You can now verify and inspect messages in the UI. Note, that there is some delay befor the UI shows the topics
   Topics > Click on Topic Name > Select Tab 'Messages' > Press plus on the left of a message to inspect content


6. To finish and shutdown all components exit run-sonar.sh and call ./stop-kafka-docker.sh

