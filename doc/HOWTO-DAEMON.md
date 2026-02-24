# Daemon mode

In the "daemon mode", Sonar stays memory-resident and pushes data to a sink.  In this mode, the only
command line parameter is the name of a config file.

The daemon is a multi-threaded system that performs system sampling, communicates with the sink, and
handles signals and lock files.

If a sink is configured - currently Kafka or a directory tree - then that is used for all data
storage.  Otherwise, data are printed on stdout.  Data formats are defined later.

The sink may also provide control messages.  The default sink reads control messages from stdin.
The directory tree sink does not read control messages.  The Kafka sink currently does not read
control messages, but this is mostly a matter of programming.  Control messages are described at the
end of this file.

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
of 24.  (Some sensible cadences such as 80m aka 1h20m, which divides the day evenly, are not
currently expressible.)

The config file has `[global]` and `[debug]` sections that control general operation; an optional
section for the transport type chosen, currently `[kafka]` or `[directory]` (otherwise terminal I/O
is used for transport); and a section each for the sonar operations, controlling their cadence and
operation in the same way as normal command line switches.  For the Sonar operations, the cadence
setting is required for the operation to be run, the command will be run at a time that is zero mod
the cadence.

### `[global]` section

```
cluster = <canonical cluster name>
role = node | master
lock-directory = <string>                       # default none
topic-prefix = <string>                         # default none
hostname-only = <bool>                          # default true
```

The `cluster` option is required, eg `fox.educloud.no`.

The `role` determines how this daemon responds to control messages from a remote controller.  It
must be defined.  Only the string values listed are accepted.  A `node` typically provides sample
and sysinfo data only, a `master` often only slurm and cluster data.

If there is a `lock-directory` then a lockfile in that directory is acquired when the daemon runs
and stays acquired for the daemon's lifetime.  If the daemon is reloaded by remote command the lock
is relinquished temporarily (and the restarted config file may name a different lock directory).

If there is a `topic-prefix` then it is prefixed to each data packet's topic.  A popular value would
be `test` to tag the data coming from test setups.  (See "DATA MESSAGE FORMATS" below for more about
topics.)  It is a bad idea to use characters other than a-z, 0-9, or hyphen within the prefix.

If `hostname-only` is set then node names are always reported as leaf names only, ie, `c1-10.fox` is
reported as `c1-10` in all contexts.  Without this setting, host names can variously be reported in
full (how the node is known to itself) or as the leaf only (how Slurm knows it), requiring the
back-end to deal with the diversity.

### `[kafka]` section

```
broker-address = <hostname and port>
rest-endpoint = <url>
http-proxy = <url>
sending-window = <duration value>               # default 5m
timeout = <duration value>                      # default 30m
ca-file = <filename>                            # default none
sasl-password = <string>                        # default none
sasl-password-file = <string>                   # default none
```

For direct communication with the Kafka broker the `broker-address` is required and provides the
address of the broker.  For Kafka it's usually host:port, eg `localhost:9092` for a local broker on
the standard port.

For communication via a REST API (usually required if traffic off the node needs to go through an
HTTP proxy), the `rest-endpoint` should be set instead of the `broker-address`, and it should be the
full URL for an API endpoint that will receive a POST with data destined for the Kafka broker.  In
this case, `ca-file` will currently be ignored - normally the URL will be https to protect the
credentials and data and the system's normal https crypto materials will be used for authenticating
the connection.  When `rest-endpoint` is used, `http-proxy` can be used to set the local proxy
address, in the event this is not set in the environment.  See the "Kafka REST proxy" section of
[HOWTO-KAFKA](HOWTO-KAFKA.md) for more.

All available data are sent to the data sink at some random time within the `sending-window`, which
starts at the point when data become available to send.

The `timeout` is how long a message is held internally without being able to be sent before it is
dropped.  The reason for sending failure could be that the broker is down, that the network is down,
and that Sonar is misconfigured.  A short timeout may be useful during debugging.

The `ca-file`, `sasl-password` and `sasl-password-file` are explained in
[HOWTO-KAFKA](HOWTO-KAFKA.md), basically the former triggers the use of TLS for the connection and
the latter two additionally add authentication.

### `[directory]` section

```
data-directory = <path>
```

The `data-directory` is required and names the root directory of the data store.  Within this
directory, there will be subdirectories following the scheme `yyyy/mm/dd` and at the bottom will be
data files for that date (as a UTC date).  The data files follow the Jobanalyzer scheme for
new-format data files: `<version>+<type>-<hostname>.json` for `sample` and `sysinfo` types
originating at node `<hostname>`; `<version>+<type>-slurm.json` for `job` (`[jobs]` section below)
and `cluzter` (`[cluster]` section below) types.  The version is currently always `0`.  The format
of the directory tree allows us to simply run Jobanalyzer on the tree later, and trees from
different nodes can be combined by recursive copy - there will be no filename conflicts at the
leaves, provided Slurm data extraction is only run on one master node, as it should be.

If the `data-directory` does not exist, Sonar will attempt to create it when producing output.  If
creation of the directory or the file fails, or a write fails, a soft error is signalled (same as if
Kafka message delivery fails).

### `[sample]` section aka `[ps]` section

```
cadence = <duration value>
exclude-system-jobs = <bool>                    # default true
load = <bool>                                   # default true
batchless = <bool>                              # default false
rollup = <bool>                                 # default false
exclude-users = <comma-separated strings>       # default []
exclude-commands = <comma-separated strings>    # default []
min-cpu-time = <duration value>                 # default none
```

These are the normal options for `sonar sample`, see the Sonar documentation.

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
batch-size = <count>                            # default none
```

The `window` is the sacct time window used for looking for data.

The `uncompleted` option, if true, triggers the inclusion of data about pending and running jobs.
This will result in multiple transmissions of data for the same `(job_id,job_step)`, one at each
sample point.  If a job stays in, say, the PENDING state for several sampling windows then multiple
transmissions for the job in the PENDING state will be seen.

If `batch-size` is set it provides the maximum number of job records per output message.  This is
basically a hack, but if `uncompleted` is true the message volume can be very large, and messages
may become so large that they cause transmission issues, notably by default Kafka limits messages to
1MB in size.  Setting `batch-size` can alleviate some of these problems.

### `[cluster]` section

```
cadence = <duration value>
on-startup = <bool>                             # default true
```

If `on-startup` is `true` then a cluster operation will be executed every time the daemon is
started, in addition to according to the cadence.

### `[programs]` section

```
curl-command = <string>                         # default "curl"
sacct-command = <string>                        # default "sacct"
scontrol-command = <string>                     # default "scontrol"
sinfo-command = <string>                        # default "sinfo"
topo-svg-command = <string>                     # default none
topo-text-command = <string>                    # default none
```

The `curl-command` is used for sending data to the Kafka REST proxy, if that is in use; see
`kafka.rest-endpoint` above.

The `sacct-command`, `scontrol-command` and `sinfo-command` commands are used to obtain slurm data
for the `[jobs]` and `[cluster]` operations.  If specified, they *must* be absolute paths without
`..` elements and spaces, or they must be empty strings (to disable).

The `topo-svg-command` value should be a command line that produces an SVG describing node topology
on stdout, typically this would be "/path/to/lstopo --of svg".  The output of the command will be
placed in the `topo_svg` field of the sysinfo blob.  Spaces separate elements and must not appear in
the commands or arguments.

The `topo-text-command` value should be a command line that produces text describing node topology
on stdout, typically this would be "/path/to/hwloc-ls".  The output of the command will be placed in
the `topo_text` field of the sysinfo blob.  Spaces separate elements and must not appear in the
commands or arguments.

Normally, at most one of `topo-svg-command` and `topo-text-command` is used.  If a command can't be
executed, the directive is silently ignored.

Note that on a serious node, the output of `lstopo` can be large, on the order of several
hundred KB when base64-encoded, while the output for `hwloc-ls` is usually quite compact.

### `[debug]` section

```
verbose = bool                                  # default false
time-limit = <duration value>                   # default none
oneshot = bool                                  # default false
output-delay = <duration value>                 # default none
```

Setting `verbose` to true will cause the daemon to print somewhat informative messages about what
it's doing at important points during the run to stderr.  (It sets the RUST_LOG level to "debug".)

Setting `time-limit` to a duration will make the daemon exit after roughly that time (it may take
longer, it will check the exit time before every sample, but not wake up from sleep just to handle
the limit).

Setting `oneshot` to true will cause the daemon to exit cleanly (and flush its output in the normal
manner) after processing a single sonar operation.

Setting `output-delay` to a duration will delay the first output until at least that much time
has passed.

## DATA MESSAGE FORMATS

Data messages are sent to a topic with a key and a value.

Data messages are sent from Sonar to the broker under topics `<cluster>.<data-type>` where
`<cluster>` is as configured in the `[global]` section and `<data-type>` is `sample`, `sysinfo`,
`job`, `cluster`.  If a topic prefix is configured, the topics become
`<prefix>.<cluster>.<data-type>`.

The key sent with a message is currently the name of the originating node, including when that node
is a master node.

The values sent with these messages are opaque.  They may be a JSON object (always new-format JSON,
see [NEW-FORMAT.md](NEW-FORMAT.md)), compressed text, and/or otherwise transformed.  Currently there
is no way of requesting anything other than JSON, and if there is compression it is applied
transparently.

When using the stdio sink, the printed data messages are JSON objects with "topic", "key",
"client", and "value" members.

## CONTROL MESSAGE FORMATS

(This is not relevant - we currently don't support control messages.  It's a design sketch only.)

Control messages are sent to Sonar under topics `<cluster>.control.<role>` where `<cluster>` is as
configured in the `[global]` section and `<role>` is `node` or `master`.  If a topic-prefix has been
set, the topics will also have to be `<prefix>.<cluster>.control.<role>`.  The messages will have
key and value as follows (very much TBD):

```
  Key     Value      Meaning
  ------- ---------- -------------------------------------------
  exit    (none)     Terminate sonar immediately
```

Control messages may be delivered more than once - just a fact of life - but will be ignored if they
are too old.

When using the stdio sink, the control messages are always single lines on the format
```
topic key value
```
For example, for a node on a cluster `hpc.xyzzy.no` with a prefix `test`, type this:
```
test.hpc.xyzzy.no.control.node exit
```

Design note: It's quite possible that the key should be either the node name or the empty string, for
messages directed at a specific node or at all, and that the command/argument should be in the
value.
