# Kafka exfiltration

In the "daemon mode", Sonar stays memory-resident and pushes data to a network sink; one of these
sinks is a Kafka broker.  See HOWTO-DAEMON.md for general information about this mode and the
options available for configuring the Kafka producer in Sonar.

See `../tests/kafka/` for examples of config files for various node and master types.

Data and control messages are as described in HOWTO-DAEMON.md, with "topic", "key" and "value"
having their standard Kafka meanings.

## CONFIGURING A STANDARD KAFKA BROKER

### Topics

For each cluster with canonical name `<cluster>` that is going to be handled by the broker, the broker needs
to be able to handle messages with these topics coming from the cluster:

```
<cluster>.sample
<cluster>.sysinfo
<cluster>.job
<cluster>.cluster
```

The broker also needs to be able to handle these control topics (tentative, may change) that are
sent from the back-end to the clients in the cluster:

```
<cluster>.control.node
<cluster>.control.master
```

### Testing with Apache Kafka

Test notes with standard Kafka server, see https://kafka.apache.org/quickstart.

#### Setup

Download the [Kafka Binary Release](https://kafka.apache.org/downloads) (.tgz).
You should download the release for scala version 2.13 and kafka version 3.9.0, since that is the only release tested with sonar.
Unpack it, once downloaded:

```
    tar xvf kafka_2.13-3.9.0.tgz
```

You're going to be running several shells, let's call them Zookeeper, Server, Consumer, Work, and Sonar.

The working directory for the following is the root directory of the unpacked Kafka distribution, e.g.,
`kafka_2.13-3.9.0/`.

NOTE!  Currently Sonar has only been tested with Kafka 3.9.0.  Kafka 4.0.0 may or may not work, it removed
support for some older protocol versions.

#### 2.13-3.9.0

In the Zookeeper shell:

```
   bin/zookeeper-server-start.sh config/zookeeper.properties
```

In the Server shell:

```
  bin/kafka-server-start.sh config/server.properties
```

In the Work shell, topics need to be added with `kafka-topics.sh` a la this, if you haven't done it
before (or if you did it, but did not shut down the broker properly, or if topics disappeared for
various random reasons):

```
  bin/kafka-topics.sh --bootstrap-server localhost:9092 \
    --create \
    --topic test-cluster.hpc.uio.no.sample
```

The topics to add are these (the last two are for control messages):

```
  test-cluster.hpc.uio.no.sample
  test-cluster.hpc.uio.no.sysinfo
  test-cluster.hpc.uio.no.job
  test-cluster.hpc.uio.no.cluster
  test-cluster.hpc.uio.no.control.node
  test-cluster.hpc.uio.no.control.master
```

#### Running sonar and examining the data

Then from the Sonar root directory, after building it, run Sonar in daemon mode with a suitable
config file in the Sonar shell:

```
  target/debug/sonar daemon tests/kafka/sonar-slurm-node.ini
```

And/or on a single node with access to slurm (eg a login node):

```
  target/debug/sonar daemon tests/kafka/sonar-slurm-master.ini
```

Sonar will run continuously and start pumping data to Kafka.

In the Consumer shell, go to `util/ingest-kafka` and build `ingest-kafka` if you haven't already.  Run
it; it will subscribe to Kafka and store messages it receives in a data store.  See instructions in
`ingest-kafka.go`.  Typical use when running on the same node as the broker with a non-standard port
XXXX would be:

```
mkdir -p data/test-cluster.hpc.uio.no
./ingest-kafka -cluster test-cluster.hpc.uio.no -data-dir data/test-cluster.hpc.uio.no -broker localhost:XXXX
```

Alternatively, for easy testing, run this in the Consumer shell to listen for sysinfo messages and echo them:

```
  bin/kafka-console-consumer.sh --bootstrap-server localhost:XXXX \
    --topic test-cluster.hpc.uio.no.sysinfo
```

Or run this in the Consumer shell to listen for `sample` messages and echo them:

```
  bin/kafka-console-consumer.sh --bootstrap-server localhost:XXXX \
    --topic test-cluster.hpc.uio.no.sample
```

#### Sending control messages

(NOTE: Not currently implemented because there's no real need for it.)

To send control messages to Sonar's compute node daemons:

```
  bin/kafka-console-producer.sh --bootstrap-server localhost:XXXX \
      --topic test-cluster.hpc.uio.no.control.node \
      --property parse.key=true
```

and then use TAB to separate key and value on each line.  A good test is `dump true` and
`dump false`, but `exit` should work (without a value).

#### Shutting down Kafka in an orderly way

In the Work shell, in the Kafka root directory:

```
bin/kafka-server-stop.sh --bootstrap-server localhost:9092
bin/zookeeper-server-stop.sh
```

#### Encryption

To enable encryption, three things must happen:

- generate and sign the necessary keys
- update Kafka's `server.properties`
- update the Sonar daemon's .ini file

We will need a CA certificate (to be used by both Kafka and Sonar) and a key store containing the
server's public and private keys (to be used by Kafka).  NOTE, the server keys will be tied to a
particular server name.

##### Testing

For testing, we will generate our own CA and key materials.  In `util/ssl`, there is a Makefile that
will generate the necessary files: `sonar-ca.crt` is the CA certificate, and
`sonar-kafka-keystore.pem` is the key store.  Just run `make all` to make the files for the local
hostname.

Having generated those, update Kafka's `server.properties` by applying
`tests/kafka/server-properties-with-ssl.diff`.  **NOTE** you may have to supply proper paths for the
keystore and CA.

The diff specifies that Kafka will continue to communicate in plaintext on port 9099 (for testing
convenience) but will communicate over TLS on port 9093.  The default port 9092 is no longer active, to
avoid confusion.

Finally, the daemon's .ini file must be updated to point the `ca-file` property to the CA certificate.  See
e.g. `tests/kafka/sonar-nonslurm-node-ssl.ini` for an example of this. **NOTE** paths may have to be
updated for your system.

##### Production

(TBD)

#### Authentication

We will use SASL PLAIN authentication over SSL for now.  In this scheme, the server's config file
contains information about security principals and their passwords.  We will use the cluster name as
the principal name (this will come into play later when we implement authorization).

We can use the same key materials we generated above for SSL, but there are additions to the config
file and the daemon's .ini file, so the correct diff for `server.properties` is now
`tests/kafka/server-properties-with-ssl-sasl.diff`.  **NOTE** you may have to supply proper paths
for the keystore and CA.

The .ini file gets an addition in the `[kafka]` section: the new `sasl-password` property must hold
the password for the cluster that is configured in the `[global]` section.  (This may change to
point to a file with that password, eventually.)  See
e.g. `tests/kafka/sonar-nonslurm-node-ssl-sasl.ini` for an example of this. **NOTE** paths may have
to be updated for your system.

#### Authorization

TODO.  Here we will use Kafka ACLs to restrict write access to topics <clustername>.<whatever> to
principals <clustername>, and maybe read access to topics <clustername>.control.<role> ditto.  This
is all very TBD.
