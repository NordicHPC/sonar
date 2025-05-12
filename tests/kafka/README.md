# Manual / interactive Kafka testing

This is a manual test case for testing sonar and the demo ingestion code with Apache Kafka 3.9.0.
It's a little hacky but not too bad.

You will need three shell windows.

In the first window, run run-kafka.sh after possibly changing its KAFKAROOT setting.

In the second window, run run-ingest.sh, it will contact Kafka and listen for traffic on various
topics.

In the third window, run run-sonar.sh, it will create data and send it to Kafka.

The scripts will create the necessary key material and compile everything as part of the process.

You should see various information about data being sent in the run-sonar window and information
about data being received in the run-ingest window.  When you eventually kill both of those (^C) you
should see the subdirectories of the "test-cluster" directory being populated with the data that
arrived.

To stop the Kafka server cleanly, run stop-kafka.sh (you may need to change KAFKAROOT).  If all else
fails, `pkill java` (and hope there's nothing else you have running that's running Java).

## Background

run-kafka.sh sets up an SSL+SASL Kafka listener on localhost:9093 and a plaintext listener on
localhost:9099.  The latter is also the control port for admin tasks.  Some topics are created for
the test-cluster.hpc.uio.no cluster.

run-ingest.sh connects to the plaintext port to consume data for those topics.

run-sonar.sh connects to the SSL+SASL port to deliver data for some of those topics.

Thus we test ssl access, plaintext access, authentication, data production, data consumption, data
formats (only valid JSON is parsed).
