# Daemon mode

In the "daemon mode", Sonar stays memory-resident and pushes data to a sink.  In this mode, the only
command line parameter is the name of a config file.

The daemon is a multi-threaded system that performs system sampling, communicates with the sink, and
handles signals and lock files.

If no other data sink is specified, the daemon prints output on stdout and reads control messages
from stdin, see later sections.

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

The config file has `[global]` and `[debug]` sections that control general operation; an optional
section for the transport type chosen, currently none; and a section each for the sonar operations,
controlling their cadence and operation in the same way as normal command line switches.  For the
Sonar operations, the cadence setting is required for the operation to be run, the command will be
run at a time that is zero mod the cadence.

### `[global]` section

```
cluster = <canonical cluster name>
role = node | master
lockdir = <string>                              # default none
topic-prefix = <string>                         # default none
```

The `cluster` option is required, eg `fox.educloud.no`.

The `role` determines how this daemon responds to control messages from a remote controller.  It
must be defined.  Only the string values listed are accepted.  A `node` typically provides sample
and sysinfo data only, a `master` often only slurm and cluster data.

If there is a `lockdir` then a lockfile in that directory is acquired when the daemon runs and stays
acquired for the daemon's lifetime.  If the daemon is reloaded by remote command the lock is
relinquished temporarily (and the restarted config file may name a different lockdir).

If there is a `topic-prefix` then it is prefixed to each data packet's topic.  A popular value would
be `test` to tag the data coming from test setups.  (See "DATA MESSAGE FORMATS" below for more about
topics.)

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
```

The `window` is the sacct time window used for looking for data.

The `uncompleted` option, if true, triggers the inclusion of data about pending and running jobs.
This will result in multiple transmissions of data for the same `(job_id,job_step)`, one at each
sample point.  If a job stays in, say, the PENDING state for several sampling windows then multiple
transmissions for the job in the PENDING state will be seen.

### `[cluster]` section

```
cadence = <duration value>
```

### `[debug]` section

```
verbose = bool                                  # default false
```

Setting `verbose` to true will cause the daemon to print somewhat informative messages about what
it's doing at important points during the run.

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

Control messages are sent to Sonar under topics `<cluster>.control.<role>` where `<cluster>` is as
configured in the `[global]` section and `<role>` is `node` or `master`.  If a topic-prefix has been set,
the topics will also have to be `<prefix>.<cluster>.control.<role>`.  The messages will have key and
value as follows (very much TBD):

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

TODO: It's quite possible that the key should be either the node name or the empty string, for
messages directed at a specific node or at all, and that the command/argument should be in the
value.
