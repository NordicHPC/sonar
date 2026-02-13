# Design, security, etc

Sonar has evolved quite a lot [over the years](HISTORICAL.md) but mostly stablized by v0.15 (2025Q3)
after the output format was redesigned, Kafka data exfiltration was added, and additional data
collection was implemented to support a sibling project. The following are the requirements guiding
the work as of 2026Q1.

Requirements on the primary Sonar functions:

- Good enough data to expose interesting behavior
- Extensible and well-defined output format
- Support for non-SLURM (indeed, non-batch-job) systems
- Support for containers
- Secure and not dependent on running as root
- Standard setup (eg as a systemd service)

Requirements on the implementation:

- Minimal overhead for recording
- Minimal overhead for data exfiltration
- Robust / fault-tolerant
- Extensible, maintainable code and minimal system dependencies

The following sections discuss these requirements in terms of what they mean, how they are met by
the existing implementation, and occasionally how and why they have evolved.

## Good enough data to expose interesting behavior

"Good enough data" means the right data, correct data, and unambiguous data.

What comprises "the right data" is tricky since it is defined by the consumer, primarily
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and its sibling project
[Slurm-monitor](https://github.com/2maz/slurm-monitor), and their use cases, which are themselves
evolving.

Over time, Sonar has evolved from simple per-process-per-node sampling to incorporate much more
per-process data, per-node hardware and OS samples, node configuration data, and companion cluster
and jobs data from SLURM, all in an effort to get "the right data".

For example, in response to requests from users we added per-CPU sampling to expose incorrect use of
hyperthreading, and we added per-process thread counts and per-node load averages which together
with node configuration data and job allocation data serve to expose imbalances in the node and
jobs - a typical case is that a node is overloaded with too many runnable threads or, equivalently,
that a job is underprovisioned on compute.

There are many other examples of data collection being added to aid specific analyses.  As the data
format is extensible (see next), more data points can be added fairly easily.

As for data being "correct" - the data must be reliable as a view of the system being monitored.
Data from one job must be properly separated from data from another job.  (This is surprisingly
involved on systems without a batch job system and no local Sonar state, although as Sonar has moved
to a daemon model it is becoming easier.)

Finally, for the data to be "unambiguous", the data must have well understood semantics.

## Extensible and well-defined output format

The JSON data structure is formally defined in terms of a Go data structure with JSON annotations
for field names and normative doc comments for documentation.  This spec is machine-processable and we use it
to generate corresponding data in several other formats as well as [user-facing documentation](NEW-FORMAT.md).

The JSON data layout was guided by [the json:api specification](https://jsonapi.org) and has proven
to be resilient in the face of many additions.

## Support for non-SLURM and non-batch-job systems

Mostly this means that a "job" concept has to
be layered on the underlying samples (this is easy), and that data analysis must be possible without
relying on batch system metadata.

## Support for containers

A job that runs inside a container (often a non-batch-system job in that
case, or a container that's being run by the batch system) must be visible to Sonar and Sonar,
running outside the container, must be able to collect meaningful data about the job.

## Secure and not dependent on running as root

We don't want to have to trust Sonar and it needs
to be possible for Sonar to collect most meaningful data without root access, running as a normal
user.  Also, any network communication of the collected data must be secure (encrypted).

## Standard setup

It must be possible to set up Sonar in a way that fits in well with the system
it is running on, ie, it needs to be possible to manage it as one-shot runs via cron, as a daemon
via systemd, and similar.

## Minimal overhead for recording

We go directly to /proc and /sys for most data, we load the
GPUs' SMI libraries for access to GPU data, and we avoid running external programs for process
sampling (which is the only performance-sensitive operation).

## Minimal overhead for data exfiltration

The expected setup is to exfiltrate via Kafka over a
secure channel.  Kafka communications can be batched and held, and the channel can remain open
between the node and the broker between communications.  If the broker is on-cluster the channel
does not need to be encrypted (probably).  Our JSON data are large but are compressed in transit at
the moment.  Should we need higher efficiency we could move to protobuf for transmission.

## Robust and fault-tolerant

Sonar must have well-understood error behavior; errors that are not
in communication itself must be detectable on the consumer side; and communication errors must be
detectable on the node.

## Extensible, maintainable code and minimal system dependencies

The code is written in portable,
idiomatic Rust with the Linux dependencies and GPU dependencies isolated in subsystems.  A cursory
study has indicated that the system is readily ported to, say, FreeBSD, and we now support four
different GPUs on two different CPU architectures.

## Security and robustness

(Fold this into the above?)

Sonar does **not** need root permissions.  It does not modify anything and writes output to stdout
(and errors to stderr) or to a user-configured directory tree or Kafka channel (with logging).

No external commands are called by `sonar sample`: Sonar reads `/proc` and `/sys` and probes the
GPUs via their manufacturers' SMI libraries to collect all data.

User-determined commands may be run by `sonar sysinfo` to extract node toplogy information.
Normally these commands are not set and the functionality is inert.  If defined, the commands are
usually `lstopo` and `hwloc-ls`, both unprivileged system commands.

The Slurm `sacct` and `scontrol` commands are currently run by `sonar jobs`, and `sinfo` is run by
`sonar cluster`.

A timeout mechanism is in place to prevent these subprocesses from hanging indefinitely.

In one-shot mode, `sonar` can use a lockfile to avoid a pile-up of processes.
