# Daemon mode and Kafka exfiltration

In the "daemon mode", Sonar stays memory-resident and pushes data to a network sink.  In this
mode, the only command line parameter is the name of a config file.

The daemon is a multi-threaded system that performs system sampling, communicates with a Kafka
broker (the network sink), and handles signals and lock files.

## CONFIG FILE

The config file is an ini-type file.  Blank lines and lines starting with '#' are ignored.  Each
section has a `[section-name]` header on a line by itself.  Within the sections, there are
`name=value` pairs where names are simple identifiers matching `/[a-zA-Z_][-a-zA-Z_0-9]*/` and values
may be quoted with `'`, `"`, or "`"; these quotes are stripped.  Blanks before and after names and
values are stripped.

Boolean values are `true` or `false`.  Duration values express a time value using the syntax `__h`,
`__m`, or `__s`, denoting hour, minute, or second values (uppercase HMS also allowed); values must
be nonzero. For cadences, second values must divide a minute evently and be < 60, minute values must
divide an hour evenly and < 60, and hour values must divide a day evenly or be a positive multiple
of 24.  (Some sensible cadences such as 90m aka 1h30m are not currently expressible.)

The config file has `[global]` and `[debug]` sections that control general operation; a section for
the transport type chosen, currently only `[kafka]`; and a section each for the sonar operations,
controlling their cadence and operation in the same way as normal command line switches.  For the
Sonar operations, the cadence setting is required for the operation to be run, the command will
be run at a time that is zero mod the cadence.

### `[global]` section

```
cluster = <canonical cluster name>
role = node | master
lockdir = <string>                              # default none
```

The cluster name is required, eg fox.educloud.no.

The role determines how this daemon responds to control messages from a remote controller.  It must
be defined.  Only the string values listed are accepted.  A `node` typically provides sample and
sysinfo data only, a `master` often only slurm and cluster data.

If there is a lockdir then a lockfile is acquired when the daemon runs and stays acquired for the
daemon's lifetime.  If the daemon is reloaded by remote command the lock is relinquished temporarily
(and the restarted config file may name a different lockdir).

### `[kafka]` section

```
broker-address = <hostname and port>
poll-interval = <duration value>                # default 5m
compression = <type>                            # default "none"
api-token-file = <path>
cert-file = <path>
key-file = <path>
ca-file = <path>
```

The `broker-address` is required and names the address of the broker.  For Kafka it's usually host:port,
eg `localhost:9092` for a local broker on the standard port.

The `poll-interval` specifies how often sonar should be polling the broker for control messages.

The `compression` can take the values `gzip`, `snappy`, or `none` and if set to something other than
`none` will attempt to enable compression of the requested type in the outgoing transmission stream.

The `api-token-file` holds an API token which, if present, is embedded into the transmitted data (in
`Meta.Token` currently but this may change) and should be specific to the cluster that is reporting.
The server will check that the token corresponds to the cluster identifier in the topic and in the
data.  If the token is used then there must be TLS.

cert-file, key-file and ca-file have to be used together and if present will force a TLS connection.

The token file and the TLS files contain secrets and must be suitably protected.

### `[sample]` section aka `[ps]` section

```
cadence = <duration value>
exclude-system-jobs = <bool>                    # default true
load = <bool>                                   # default true
batchless = <bool>                              # default false
exclude-users = <comma-separated strings>       # default []
exclude-commands = <comma-separated strings>    # default []
```

These are the normal options for `sonar ps`, see the Sonar documentation.

### `[sysinfo]` section

```
cadence = <duration value>
on-startup = <bool>                             # default true
```

If `on-startup` is `true` then a sysinfo operation will be executed every time the daemon is
started, in addition to according to the cadence.

### `[jobs]` section aka `[slurm]` section

```
cadence = <duration value>
window = <duration value>                       # default 2*cadence
uncompleted = <bool>                            # default false
delta-coding = <bool>                           # default true
dump = <filename>
```

The window is the sacct time window used for looking for data.

The `uncompleted` option, if true, triggers the inclusion of data about pending and running jobs.
This will result in multiple transmissions of data for the same `(job_id,job_step)`, one at each
sample point.  If a job stays in, say, the PENDING state for several sampling windows then multiple
transmissions for the job in the PENDING state will be seen.

The `delta-coding` option, if true, triggers optimization of data transmission: redundant data are
omitted in subsequent transmissions, and if there are no pertient data changes - for example, if a
new `PENDING` record has the same contents as one already sent because no settings have changed -
then the second record will not be sent at all.  What is deemed pertient or redundant is up for
discussion for both `PENDING` and `RUNNING` jobs.  In any case, the recipient must be prepared to
reconstruct the complete data stream from the initial record for the `(job_id,job_step)` and any
subsequent deltas, applied in order.  If `uncompleted` is true, the `delta-coding` option can
greatly reduce transmitted data volume.  If `uncompleted` is false, it can still avoid redundantly
sending information about completed jobs.

Setting `dump` to a file name will cause JSON output to be appended to the given file, while also
being sent in the normal way.  If appending fails, a diagnostic is printed if `[debug]:verbose` is
true.

### `[cluster]` section

```
cadence = <duration value>
```

### `[debug]` section

```
dump = bool                                     # default false
verbose = bool                                  # default false
```

Setting `dump` to true will cause the daemon to dump all data it is sending on stdout.

Setting `verbose` to true will cause the daemon to print somewhat informative messages about what
it's doing at important points during the run.

### Example config files

See `../util/ingest-kafka/` for examples of config files for various node and master types.

## DATA MESSAGE FORMATS

Kafka messages are sent to a topic with a key and a value.

Data messages are sent from Sonar to the broker under topics `<cluster>.<data-type>` where `<cluster>`
is as configured in the `[global]` section and `<data-type>` is `sample`, `sysinfo`, `job`, `cluster`.

The key sent with a message is currently the name of the originating node, including when that node
is a master node.

The values sent with these messages are opaque.  They may be a JSON object (always new-format JSON,
see [NEW-FORMAT.md](NEW-FORMAT.md)), compressed text, and/or encrypted in some manner.  Currently
there is no way of requesting anything other than JSON; if there is compression it is applied
transparently, and if there is encryption it is by TLS, which must be configured explicitly in the
config file.

## CONTROL MESSAGE FORMATS

Control messages are sent from the backend via the broker to Sonar under topics
`<cluster>.control.<role>` where `<cluster>` is as configured in the `[global]` section and `<role>`
is `node` or `master`.  These will have key and value as follows (very much TBD):

```
  Key     Value      Meaning
  ------- ---------- -------------------------------------------
  exit    (none)     Terminate sonar immediately
  dump    <boolean>  Enable or disable data dump (for debugging)
```

TODO: The whole thing with control messages is a little dicey.  We would like to have mechanisms
whereby "older" messages expire at some time but "newer" messages are held for a time so that they
are seen by a node that comes up after the message is sent but before the message expires.

TODO: It's quite possible that the key should be either the node name or the empty string, for
messages directed at a specific node or at all, and that the command/argument should be in the
value.

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

You're going to be running several shells.

The working directory for the following is the root directory of the unpacked Kafka distribution, eg
`kafka_2.13-3.9.0/`.

In the first shell:

```
   bin/zookeeper-server-start.sh config/zookeeper.properties
```

In the second shell:

```
  bin/kafka-server-start.sh config/server.properties
```

In the third shell, topics need to be added with `kafka-topics.sh` a la this, if you haven't done it
before (or if you did it, but did not shut down the broker properly):

```
  bin/kafka-topics.sh --create --topic fox.educloud.no.sample --bootstrap-server localhost:9092
```

The topics to add are these (the last two are for control messages):

```
  fox.educloud.no.sample
  fox.educloud.no.sysinfo
  fox.educloud.no.job
  fox.educloud.no.cluster
  fox.educloud.no.control.node
  fox.educloud.no.control.master
```

#### Running sonar and examining the data

Then from the Sonar root directory, after building it, run Sonar in daemon mode with a suitable
config file:

```
  target/debug/sonar daemon util/ingest-kafka/sonar-slurm-node.cfg
```

And/or on a single node with access to slurm (eg a login node):

```
  target/debug/sonar daemon util/ingest-kafka/sonar-slurm-master.cfg
```

Sonar will run continuously and start pumping data to Kafka.

In a fourth shell, go to `util/ingest-kafka` and build `ingest-kafka` if you haven't already.  Run
it; it will subscribe to Kafka and store messages it receives in a data store.  See instructions in
`ingest-kafka.go`.  Typical use when running on the same node as the broker with a non-standard port
XXXX would be:

```
mkdir -p data/fox.educloud.no
./ingest-kafka -cluster fox.educloud.no -data-dir data/fox.educloud.no -broker localhost:XXXX
```

Alternatively, for easy testing, run this in a shell to listen for sysinfo messages and echo them:

```
  bin/kafka-console-consumer.sh --topic 'fox.educloud.no.sysinfo' --bootstrap-server localhost:XXXX
```

Or run this in a shell to listen for sample messages and echo them:

```
  bin/kafka-console-consumer.sh --topic 'fox.educloud.no.sample' --bootstrap-server localhost:XXXX
```

#### Sending control messages

To send control messages to Sonar's compute node daemons:

```
  bin/kafka-console-producer.sh --bootstrap-server localhost:XXXX --topic fox.educloud.no.control.node --property parse.key=true
```

and then use TAB to separate key and value on each line.  A good test is `dump true` and
`dump false`, but `exit` should work (without a value).

#### Shutting down Kafka in an orderly way

In any shell in the Kafka root directory:

```
bin/kafka-server-stop.sh --bootstrap-server localhost:9092
bin/zookeeper-server-stop.sh
```
