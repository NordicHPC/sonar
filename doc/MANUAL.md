# Sonar User Manual

## Introduction

Sonar is an unprivileged Linux system daemon that captures a lightweight profile of processes,
accelerator cards, disks, nodes, jobs, and clusters.  It runs on compute nodes to capture profiles
of user processes and node states, and on master nodes to capture snapshots of job queues and
cluster status.  It is always running and can be used for post-hoc and on-line analyses of jobs and
system usage from a system user's point of view.

In normal operation, Sonar exfiltrates its data to a central data aggregator, often off-cluster,
where they are stored in a database that can be mined for further insight.  [Companion
tools](#companion-tools) exist for data access and analysis.

Alternatively, data can be stored on-node in a directory tree; this is mostly useful for single-node
non-Slurm configurations, as in some cloud deployments.

Sonar operation is driven by a configuration file that determines how and how often to collect and
exfiltrate the data.


## Normal operation

In normal operation, Sonar is installed as a systemd service that simply runs `sonar daemon
config-file` as an unprivileged user; Sonar then stays memory-resident and maintains a modest amount
of internal state.  See [Systemd operation](#systemd-operation) below.

The configuration file is an .ini type file that has three broad parts, described below: a
[general part](#configuration-the-general-part), a [data sink part](#configuration-the-data-sink-part),
and an [operation part](#configuration-the-operation-part).

The configuration file uses `#` for a comment character; string values can be quoted or not; boolean
values are `true` or `false`; and duration values are a number followed by a letter: `30s`, `5m`,
`12h`.

The following manual sections discuss the most common configuration settings; see
[HOWTO-DAEMON.md](HOWTO-DAEMON.md) for more obscure settings and a more complete guide to .ini
syntax.

### Configuration: the general part

#### Global section

The `[global]` section configures Sonar generally.

The `cluster` setting is the canonical name of the cluster and is required.  For HPC clusters this
is usually a well-known cluster name such as `saga.sigma2.no`.  For an ad-hoc cluster of nodes, any
name can be given to the cluster; usually it takes the form of a domain name, but it need not.

The `role` setting is either `node` for a compute node or `master` for a cluster master node.  The
setting is required.

Example:
```
[global]
cluster = saga.sigma2.no
role = node
```

#### Programs section

The `[programs]` section defines path names for helper programs that are used by some operations and
some forms of data exfiltration.  In general, values should be absolute paths without `..` elements
and use spaces only to separate commands and arguments.  When sensible, an empty string value
disables the use of the program.

The `curl-command`, by default `curl`, is used for sending data to the Kafka HTTP proxy, if that is
in use.

The `sacct-command`, `scontrol-command` and `sinfo-command` commands, by default `sacct`,
`scontrol`, and `sinfo`, are used to obtain slurm data for the `[jobs]` and `[cluster]` operations.

The `topo-svg-command` and `topo-text-command`, by default nothing, should be commands that produce
an SVG or some text describing node topology on stdout, for inclusion in sysinfo data.  Typical
values would be `/path/to/lstopo --of svg` and `/path/to/hwloc-ls`.  Normally, at most one of the
values would be specified.  On a serious node, the output of `lstopo` can be very large.

Example:
```
[programs]
curl-command = /home/me/bin/curl
topo-text-command = /cluster/software/EL9/easybuild/software/hwloc/2.11.2-GCCcore-14.2.0/bin/hwloc-ls
```

### Configuration: the data sink part

The data sink is either a remote aggregator, specified via the `[kafka]` section, a local directory
tree, specified via the `[directory]` section, or standard output, if neither of the other sections
are present.

#### Kafka exfiltration

In the typical case, data are sent to an off-cluster Apache Kafka broker, whence the data aggregator
will fetch them.  The connection from Sonar to the broker can be direct, or if the Sonar node is
behind an HTTP proxy, via a Kafka HTTP proxy.  These two cases share most configuration settings.
The section is called `[kafka]`.

##### Direct broker connection

For direct broker access, the address of the broker (hostname and port) is given by the
`broker-address` setting, and the file name of the certificate that is used for the TLS connection
between Sonar and broker is given by the `ca-file` setting.  It's usually easiest to construct one's own [crypto materials](#crypto-materials).

Example:
```
[kafka]
broker-address = my-aggregator.uio.no:1234
ca-file = /var/lib/sonar/secrets/my-aggregator-ca.crt
```

##### HTTP proxy connection

For proxied access, the address of the Kafka HTTP proxy endpoint (a URL that will receive a POST) is
given by `rest-endpoint`, and if the local HTTP proxy settings are not given by the environment in
which Sonar is running then they can be specified using the `http-proxy` setting.

Example:
```bash
[kafka]
rest-endpoint = https://my-aggregator.uio.no/kprox
http-proxy = http://proxy.saga:1234/
```

The protocol used for communicating with the Kafka HTTP proxy is custom and requires a custom proxy
implementation; see [util/kafka-proxy/kprox.go](../util/kafka-proxy/kprox.go) in this repository for
a simple implementation and how to use it, as well as a definition of the custom protocol.

##### Shared Kafka settings

Additional settings for the Kafka exfiltration are shared between the two methods.  The most
important setting is the data upload password, which can be specified in-line with `sasl-password`
or out-of-line with `sasl-password-file` (the recommended approach).  The password file must contain
the password by itself on the first line.

Additionally, the `sending-window` setting specifies an interval during which the time to send the
data is chosen randomly (to spread out traffic at the receiver), it defaults to `5m`.

The `timeout` setting specifies how long a message can be held without being sent successfully
before it is discarded, it defaults to `30m`.

Example:
```
[kafka]
sending-window = 4m
timeout = 1h
sasl-password-file = /var/lib/sonar/secrets/my-aggregator-upload-password.txt
```

#### Output to directory tree

For output to a directory tree, the root of the tree in the local file system is specified using the
`data-directory` setting.  Conventionally, the last element of the directory name is the cluster
name but this is not required.  Under that directory there will be a tree with paths of the form
`yyyy/mm/dd` and inside the bottom directory there will be files containing data for that date, one
file per data type per day.  See [HOWTO-DAEMON.md](HOWTO-DAEMON.md) for more about the naming of the
files, and [Data](#data) for more about the data themselves.  Existing analysis tools can
operate directly on the tree as a database.

Example:
```
[directory]
data-directory = /var/log/sonar/my-cluster.uio.no
```

### Configuration: the operation part

There are four operations, known as `sample`, `sysinfo`, `jobs`, and `cluster`.  The first two
normally run only on compute nodes (`global.role` is `node`), the latter two on the slurm master
node.

For all operation types, the `cadence` attribute must be set to indicate how often the operation
should run.  The operation runs at a time divisible by the cadence (a 15-minute cadence means that
the operation runs on the hour and at at a quarter, a half, and three quarters after the hour).  A
second cadence must divide a minute evenly, a minute cadence an hour evenly, and an hour cadence a
day evenly or be a multiple of days.

Below, only the most useful attributes are listed, for others see [HOWTO-DAEMON.md](HOWTO-DAEMON.md).

#### The `sample` operation

The `sample` operation runs fairly frequently (from every 30s to every 5m is sensible) and snapshots
information about the state of processes, memories, cores, cards, and disks.

The settings for `sample` are mostly about controlling the data volume.

The `exclude-system-jobs` attribute can be set to false to also collect information about processes
owned by system users, notably root.  This is useful on standalone VM nodes where processes may run
inside Docker containers, where they often run as root.

The `exclude-commands` attribute can be set to a list of command names to ignore.  A typical value
might be a list of shell names, as shells are rarely of interest for later analysis.  The default is
an empty list.

The `min-cpu-time` attribute can be set to not report processes and jobs whose cumulative CPU time
is less than the value; a value of 60s seems to cull many uninteresting things.  The default is
zero.

The `rollup` attribute can be set to true to merge processes that have no children, have the same
parent, and have the same command name, into a single pseudo-process.  This is especially meaningful
for compressing data on large HPC nodes running MPI jobs.  However, some nuance in the data is
unavoidably lost.

Example:
```
[sample]
cadence = 5m
min-cpu-time = 60s
exclude-commands = bash,ssh,zsh,tmux,systemd
rollup = true
```

#### The `sysinfo` operation

The `sysinfo` operation runs infrequently (every 12h or 24h plus at service start) and reports on
the current system configuration.  The system configuration does not change much, but it can change
due to hot-swapping components, reboots, and various on-line reconfiguring.  It is thus not
important to capture it minute-by-minute but "occasionally".

The `sysinfo` operation does not have any interesting attributes beyond `cadence`.

Example:
```
[sysinfo]
cadence = 12h
```

#### The `jobs` operation

The `jobs` operation is the slurm master dual to `sample`: it captures the current state of slurm
jobs from the job manager's point of view.  To do this, it extracts the state of all pending,
running, and recently completed jobs on the cluster.  The data volume can be significant and it may
be important to find a good tradeoff between the data volume and overhead of extracting it, and the
precision of subsequent analysis.  A good cadence may be around 15m.

The `window` attribute sets the time window for interactions with the slurm database.  By default it
is twice the cadence.

The `uncompleted` attribute should be set to true to capture pending and running jobs, though by
default it is false.  Adding pending and running jobs can significantly increase the data volume,
but the data are of interest if the Sonar data are being used to analyze ongoing (and not just
completed) activity.

The `batch-size` attribute is used to divide up the data into packets of a sensible size, and this
may be particularly important if `uncompleted` is true.  Too-large packets may be rejected by a
Kafka broker if it has not been configured correctly.  A value of 500 seems to fit well with the
default Kafka packet limit of 1MB.

Example:
```
[jobs]
cadence = 15m
uncompleted = true
batch-size = 500
```

#### The `cluster` operation

The `cluster` operation is the slurm master dual to `sysinfo`: it captures the current state of
slurm nodes and partitions.  The data volume is modest and these values do not change very often,
so a cadence of every few hours (plus at service start) is usually good.

The `cluster` operation does not have any interesting attributes beyond `cadence`.

Example:
```
[cluster]
cadence = 4h
```

### Systemd operation

A typical installation has a user `sonar` in a group `sonar` and installs the Sonar files in
`/usr/local/lib/sonar`.  In that location is the Sonar binary, the Sonar config file, an maybe a
subdirectory called `secrets` with the password file and the TLS server certificate for
communicating with the broker.  In addition, there is a systemd service file, which might look like
this:

```
[Unit]
Description=Sonar continuous profiling service
Documentation=https://github.com/NordicHPC/sonar
After=local-fs.target remote-fs.target network.target

[Service]
User=sonar
Group=sonar
ExecStart=/usr/local/lib/sonar/sonar daemon /usr/local/lib/sonar/sonar.cfg
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

If SELinux is enabled then the files *must* be in `/usr/local/lib` and the User and Group directives
in the service filw won't be honored.  Instead, use this workaround:

```
ExecStart=/usr/sbin/runuser -u sonar -- \
    /usr/local/lib/sonar/sonar daemon /usr/local/lib/sonar/sonar.cfg
```

## Data

### Formats

A number of different kinds of data are being collected, but in transit they are sent as four
different types of packets, known as Sample, Sysinfo, Jobs, and Cluster packets, corresponding to
the four operations that collect the data.  They are specified as JSON data structures, and are
currently also transmitted as JSON textual data.

The four packet types are defined in [NEW-FORMAT.md](NEW-FORMAT.md) by the type definitions
[SampleEnvelope](NEW-FORMAT.md#type-sampleenvelope),
[SysinfoEnvelope](NEW-FORMAT.md#type-sysinfoenvelope),
[JobsEnvelope](NEW-FORMAT.md#type-jobsenvelope), and
[ClusterEnvelope](NEW-FORMAT.md#type-clusterenvelope), along with their accompanying comments, all
of which is normative.

It is possible to run Sonar operations from the command line without a config file and with output
to stdout, so as to examine and understand the data more easily.  See [Advanced operation](#advanced-operation).

### Kafka topics

Kafka messages are sent to *topics*, with a *key* and a *value*.  The value is always a JSON data
packet as described above.  The key is the name of the host sending the message.  The topic is
always `<cluster-name>.<packet-type>` where `cluster-name` is the name of the cluster from the
`global` section and `packet-type` is `sample`, `sysinfo`, `job` [sic!], or `cluster`.  The
`packet-type` always corresponds to the type of data being transmitted in the value.


## Advanced operation

### One-shot execution

The individual Sonar operations can also be executed interactively without a configuration file.
This is useful for debugging or if Sonar has to be run as a cron job instead of a systemd service.

Stdout output and one-shot operation may be useful if data has to be exfiltrated in some custom
manner, for example, by using curl to send the data to some custom aggregator.

To run Sonar in one-shot mode, provide the operation name as the first argument, and then further
refine it with options, e.g.,

```
sonar sample --exclude-system-jobs --min-cpu-time 60
```

Run `sonar help` for more help on options.  Note that some defaults differ from daemon mode, and some
values have to be specified differently (eg, the value `60` above would be `60s` in the config file).

### Daemon debugging

A special `[debug]` section can be used to ask for verbose output and some features that allow
for easier testing.  See [HOWTO-DAEMON.md](HOWTO-DAEMON.md).

## Older output formats

As of v0.17 there is only one output format, known as [the new format](doc/NEW-FORMAT.md), a JSON
encoding.  Previously there were other formats, notably a CSV format and a different JSON encoding.
Older data files may still use these formats.  Check out the documentation (and code) on the
`release_0_16` branch to learn more about that, should you need it.

## Crypto materials

Use [../util/ssl/Makefile](../util/ssl/Makefile) to generate your own crypto materials if you need
them for communicating between Sonar and the Kafka broker.  You want the sonar-ca.crt file that is
generated by that process.  The `HOSTNAME` must be the name of the host that is going to be running
the broker.


## Companion tools

Prototype tools to work with Sonar data exist in the form of
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and
[slurm-monitor](https://github.com/2maz/slurm-monitor).  These can be set up to ingest data from a
Kafka broker into a database, and provide direct query facilities against the database.  Both also
provide Web-based dashboards with pre-built analyses.


## Building Sonar

### Casual builds

Sonar is written in Rust (with some C).  If you have a recent Cargo and Rust installed locally, and
you're on an x86_64 or aarch64 Linux system, simply run `make` in the top-level directory to build
with default features and using pre-built shims for interaction with accelerator cards (currently
NVIDIA, AMD, Intel XPU and Intel Habana).  The executable appears as `target/release/sonar`.

### Building for development

See [HOWTO-DEVELOP.md](HOWTO-DEVELOP.md#compilation) for all instructions.

### Building RPMs

See [HOWTO-DEVELOP.md](HOWTO-DEVELOP.md#rpm-builds) for RPM-specific instructions.
