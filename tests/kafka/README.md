# Manual / interactive Kafka testing

This is a manual test case for testing sonar and the demo ingestion code with Apache Kafka 3.9.0.
It's a little hacky but not too bad.

## Preliminaries

### Kafka

You do not need to *install* Kafka but you must have a Kafka distro downloaded.  Currently we
*require* Kafka 3.9.0.  Get it from [the official site](https://kafka.apache.org/downloads).

After downloading, untar it.  The default for the tests below is that it is untarred in `~/lib`, so
your Kafka working directory (`$KAFKAROOT`) will be `~/lib/kafka_2.13-3.9.0`.

### Rust

You will need Rust, but you probably have that already since you have this repo.

### Go

You will need a recent (1.22.1 at the time of writing) version of Go to compile one of the test
programs.

### Openssl

You will need openssl to generate key materials, install it in your distro's normal manner.

## Running the tests

You will need three shell windows.  Do the following in order:

In the first window, run `run-kafka.sh` after possibly changing its `KAFKAROOT` setting.

In the second window, run `run-ingest.sh`, it will contact Kafka and listen for traffic on various
topics.

In the third window, run `run-sonar.sh`, it will create data and send it to Kafka.

The scripts will create the necessary key material and compile everything as part of the process.

You should see various information about data being sent in the run-sonar window and information
about data being received in the run-ingest window.  When you eventually kill both of those (^C) you
should see the subdirectories of the `test-cluster` directory in the present directory having being
populated with the data that arrived.

To stop the Kafka server cleanly, run `stop-kafka.sh` (you may need to change its `KAFKAROOT`).  If
all else fails, `pkill java` (and hope there's nothing else you have running that's running Java).

## Background

`run-kafka.sh` sets up an SSL+SASL Kafka listener on `localhost:9093` and a plaintext listener on
`localhost:9099`.  The latter is also the control port for admin tasks.  Some topics are created for
the `test-cluster.hpc.uio.no` cluster.

`run-ingest.sh` connects to the plaintext port to consume data for those topics.

`run-sonar.sh` connects to the SSL+SASL port to deliver data for some of those topics.

Thus we test ssl access, plaintext access, authentication, data production, data consumption, data
formats (only valid JSON is parsed).
