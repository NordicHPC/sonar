# Sonar requirements and high-level design issues

This document discusses high-level requirements and some of the high-level design and implementation
decisions that are implied by those requirements.  The structure of the source code tree and lower
level implementation matters are discussed in [HOWTO-DEVELOP.md](HOWTO-DEVELOP.md), otherwise UTSL.

Sonar has evolved quite a lot [over the years](HISTORICAL.md) but mostly stabilized by v0.15 (2025Q3)
after the output format was redesigned, Kafka data exfiltration was added, and additional data
collection was implemented to support a sibling project. The following are the requirements guiding
the work as of 2026Q1.


## Purpose and overall primary requirement

The primary requirement is that Sonar should collect data from the nodes of an HPC system, and from
the system itself, with the purpose of allowing the data to be analyzed so as to expose information
about users and jobs on the system that might cause system underutilization.

Underutilization can be caused by jobs not using allocated resources, misbalancing processes and
ranks, overutilizing available resources (thrashing), and similar issues, but also by choosing the
wrong system, queue, or accelerator for the job.  The list is not exhaustive.

Sonar differs from tools like Seff in two ways: first, in that it collects a continous profile of
the individual processes of user jobs in addition to overall job data, and allows detailed job
behavior to be examined post-hoc.  Second, Sonar is itself not the analysis tool, only the data
collector.  Analysis falls to companion tools (see later for examples).

Sonar's intended users are partly second-line support staff, partly sysadmins, and partly the users
themselves, who can be exposed to (processed) Sonar data in various ways, not germane here.


## High-level requirements

In the context of the primary requirement, the high-level requirements on the primary Sonar
functions (data collection and exfiltration) are:

- Good enough data to expose interesting behavior
- Extensible and well-defined output format
- Support for non-SLURM and non-batch-job systems
- Support for containers, including Docker
- Support for local data aggregation
- Secure and not dependent on running as root
- Standard setup (eg as a systemd service)
- Reuse existing infrastructure when possible
- Convenient and easy to use

There are additionally some high-level requirements on the implementation:

- Minimal overhead for recording
- Minimal overhead for data exfiltration
- Robust / fault-tolerant
- Extensible, maintainable code and minimal system dependencies
- Testable
- Aware of supply chain security issues

The following sections discuss these requirements in terms of what they mean, how they are met by
the existing implementation, and occasionally how and why they have evolved.


### Good enough data to expose interesting behavior

"Good enough data" means the right data, correct data, and unambiguous data.

The data must be "the right data", but what comprises the right data is tricky since it is defined
by the consumer and their use cases.  Currently the primary consumers are
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and its sibling project
[Slurm-monitor](https://github.com/2maz/slurm-monitor), and these are themselves evolving.

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

The data must be "the correct data", that is, must provide a reliable view of the system being
monitored.  Data from one job must be properly separated from data from another job, and data from
multiple strands of the same job (ranks, subcommands) must be relatable to the job.  (This is
surprisingly involved on systems without a batch job system and no local Sonar state, although as
Sonar has moved to a daemon model it is becoming easier.)

Finally, the data must be "unambiguous data", with well understood semantics.  This is harder than
it might seem since frequently the data from the Linux kernel, from the GPU cards, and from I/O
devices are underspecified and themselves ambiguous.  We try to fix this problem by being very
explicit in the data definition about the data semantics and grounding the semantics in kernel
source, kernel and device documentation, and the source of existing libraries such as `psutil`.


### Extensible and well-defined output format

We use JSON for all data at the moment.  JSON is easily processable in almost any programming
language and in addition by `jq` at the command line, facilitating easy testing.

(Earlier iterations of Sonar produced CSV but the non-hierarchical nature of CSV led to redundancy,
workarounds for nesting fields within others, questionable extensibility, and larger data overall.)

The JSON data layout is formally defined in terms of a Go data structure with JSON annotations for
field names and normative doc comments for documentation.  The spec is machine-processable and we
use it to generate corresponding data in several other formats, as well as [user-facing
documentation](NEW-FORMAT.md).

The JSON data layout was guided by [the json:api specification](https://jsonapi.org) and has proven
to be resilient in the face of many additions.

The JSON is not exactly compact however, and parsing text can be relatively costly.  The
hierarchical tree nature of JSON is easily translated to faster and more compact formats such as
protobuf or fastbuf, should we need to do that in the future.  At the moment, data volumes are not
such that that is necessary.


### Support for non-SLURM and non-batch-job systems

While many HPC systems run Slurm (or something like it), not all do.  At UiO we have a clutch of
"light HPC" many-core and GPU-enabled systems where users can just run jobs; there are also VM nodes
(as managed by the NAIC Orchestrator) where there is no batch system but it is interesting to
collect Sonar data for analysis.  Sonar must thus not be dependent on a batch system but must handle
the idea of a "job" in a non-batch setting.

**Implementation:**

Linux already has an idea of a "job" - it is a process group.  A little more challenging is that we
will have no idea about what the job *should* have done, as there is no job script with resource
requests or similar.  This is not a problem for Sonar, but it does impose a requirement that Sonar
record the process group and that it not require the availability of eg a Slurm Job ID in the cgroup
data for a process.


### Support for containers, including Docker

It's becoming normal for HPC jobs to run in containers, notably Singularity.  On non-batch and
unshared nodes, such as in VMs managed by NAIC Orchestrator, it is also common to run workloads in
Docker.  Sonar needs to be able to collect data about jobs running in these containers while itself
running outside the container.

**Implementation:**

For Singularity this appears to be a non-issue, its processes are visible as normal user processes.
For Docker it depends: Docker processes will run as root when started with `docker run`, and it may
be hard to connect them to a particular "job" or user (and additionally, Sonar would normally be
configured to ignore processes owned by root to cut down the data volume; including root jobs has an
efficiency component).  If multiple users run Docker processes at the same time they are easily
confused.  We could consider some way for `sonar jobs` to collect Docker information, maybe, if we
find some way to reveal more information, but at first blush even `docker stats` does not really
reveal what's running.


### Support for local data aggregation

Especially for cloud installations such as VMs managed by NAIC Orchestrator it is desirable to avoid
any centralized data aggregation, as that might not easily scale, but instead to allow data on the
node to be aggregated on the node in a format that is easily transportable and extractable before
the node is torn down.

**Implementation:**

Sonar can be told to aggregate the data in a standard format in a directory tree, which can be
zipped up and copied.


### Secure and not dependent on running as root

We don't want to have to trust Sonar or the code that went into it (see later about supply chain
security), so it needs to be possible for Sonar to collect all meaningful data without root or other
privileged access, that is, running as a normal user.

Also, any network communication of the collected data must be secure (encrypted) since the data are
a little bit sensitive.

It must not be possible for a node not on the cluster to submit data for the cluster.

Any subprocesses should be time-limited, should be used sparingly, should not need to be privileged,
and should (within reason) be possible to disable.

If special group membership is needed to access the GPUs (eg if a card is owned by `video`) then it
is acceptable to require Sonar to be in this group.

**Implementation:**

User-determined commands may be run by `sonar sysinfo` to extract node toplogy information.
Normally these commands are not set and the functionality is inert.

The Slurm `sacct` and `scontrol` commands are currently run by `sonar jobs`, and `sinfo` is run by
`sonar cluster`.  These can be set to other values than the default but cannot be entirely disabled
except by pointing to a program that produces no output.

A timeout mechanism is in place to prevent the preceding subprocesses from hanging indefinitely.

Sonar currently uses `curl` to exfiltrate data to the Kafka HTTP proxy, if that is in use.

With regard to data provenance, each cluster has a dedicated upload password that must remain secret
that all the nodes use to submit data.  The Kafka broker must be set up to check this password for
the cluster name / Kafka topic.  This is considered a "good enough" solution, it adds a layer of
security but not so much management overhead that managing the cluster becomes a hardship.

Finally, a TLS certificate must be installed for encrypted communication with the Kafka broker.  If
the Kafka HTTP proxy is used then the most sensible setup will use an HTTPS upload point (and the
proxy will engage in the encrypted, password-checked communication with the broker).


### Standard setup

It must be possible to set up Sonar in a way that fits in well with the system it is running on, ie,
it needs to be possible to manage it both as a daemon via systemd, or as one-shot runs managed by
cron, if that is desirable.

**Implementation:**

Initially Sonar was one-shot only, with only one kind of data being collected and printed on stdout.
That one-shot mode persists because it is very useful for testing and experimentation, but for
production the daemon mode with exfiltration to Kafka or some directory tree (see above under local
aggregation) is the way to go.


### Reuse existing infrastructure when possible

Sonar should rely on existing technologies and not invent its own.

**Implementation:**

To this end, Sonar uses Kafka with TLS or plain HTTPS to a Kafka HTTP proxy for data exfiltration.
Kafka is a sensible choice here as it is a reliable store-and-forward technology.  Earlier versions
of Sonar used a custom HTTPS POST protocol to a custom back-end.

That said, Sonar has its own Kafka HTTP proxy (we found none that suited our needs when those arose)
and Sonar is doing all our own information collection, not relying on libraries such as `psutil` for
example.


### Convenient and easy to use

It needs to be possible to use Sonar in various settings: exploration, production, and testing.  And
it needs to be possible to manage Sonar installation at a site without having to recompile it everywhere.

**Implementation:**

In its one-shot mode, Sonar allows one to experiment with parameters and settings and to figure out
how best to process the output.  There is command-line help (`sonar help`) to aid this exploration.
This is also a good mode for tests, which can process the output with `jq`.

In contrast, the daemon mode is best for production even if it, too, has test features (debug
settings in the config file and some environment variables that are honored in debug builds).

Sonar can be compiled with support for all GPUs enabled, and will probe for GPUs at run-time.  Or it
can be compiled with support only for GPUs available at the site.


### Minimal overhead for recording

Sonar must avoid running helper programs except in cases where they are expected to be run rarely
and the output can be controlled tightly.

For frequent operations Sonar must go directly to system databases (/proc, /sys, and system calls)
for system data, and must use the GPUs' SMI libraries directly for access to GPU data.

The background for those requirements is that early versions of Sonar would run `ps` and parse the
output, an expensive operation.  Sonar would also run the GPUs' SMI programs and parse the output of
those, which was not only expensive but limiting (the SMI programs did not expose everything we
wanted) and fraught with compatibility issues (the output of the SMI programs was not stable).  The
combined pain from the cost of running subprograms that produced text and the cost and headaches of
parsing that output was too much to bear.

**Implementation:**

Sonar can be linked with zero or more GPU shims for various GPUs, and at runtime these shims will
attempt to determine if the GPUs are installed and if so try to load the SMI shared objects.  A
Sonar binary is GPU-aware, but not GPU-dependent.


### Minimal overhead for data exfiltration

Data exfiltration overhead can be incurred on the node, internally on the cluster's local network,
and on the external net from the cluster to the data collector.

Sonar should strive to minimize the overhead incurred on each of those components by allowing data
to be communicated efficiently using at least a direct-to-aggregator path (necessarily somewhat
expensive) and a indirect-via-intermediary path (can be cheaper if it offloads the node and the local
network).

At the same time, we want data delivery to be reliable so that there is minimal data loss, basically
requiring TCP and precluding UDP.

As touched upon in the requirement about reusing existing infrastructure, above, Sonar currently
prefers to use exfiltration via the Apache Kafka protocol, a reliable store-and-forward system.
This can be set up with a broker off-cluster with TLS data upload, typically incurring a fresh
connection from each node per data item both on the local network and off-cluster, or it can be set
up with a broker on-cluster, possibly without encryption, that then forwards all data to the
off-cluster broker over a single, pretty much permanent connection.

A reality on many HPC systems is that the nodes are behind an HTTP proxy, precluding the use of
Kafka unless additional ports are opened (as Kafka uses TLS but not HTTPS) or a Kafka HTTP proxy is
employed to receive the traffic and forward it.  Sonar can use either method; it includes such a
proxy.

Kafka employs fast compression normally, but if the data turn out to be too large still, we can move
from JSON to a protobuf or fastbuf data representation.

**Implementation:**

Sonar currently uses `curl` to send data to the Kafka HTTP proxy over HTTP(S).  The `curl` process
is spawned with retry/timeout parameters without Sonar waiting for it to complete.  Using `curl` is
slightly more expensive than using a built-in solution.


### Robust and fault-tolerant

Robustness and fault tolerance has multiple aspects.

Sonar itself should not crash and should not produce garbage results.  If Sonar itself is in an
error state it should either communicate the error or log the error if communication is not working.

When a node is in an error state, Sonar should produce an error report about the node that can be
processed by the aggregator or back-end.

Additionally, data should not be lost.  This means that if data cannot be sent, sending should be
retried for some time.  And if data is sent, there should be a sensible chance that the receiver
will not lose them.

**Implementation:**

The Sonar daemon tries hard not to exit unless told to (by a signal).  The data format has ample
space for error reports at both the global and local level and Sonar will report errors through
this channel.

By default, data is held for a half hour before it is consider undeliverable (the limit can be set
higher).  This is the case whether the data are sent by native Kafka or via `curl` to the Kafka HTTP
proxy.

The use of Kafka adds reliability, as Kafka is both store-and-forward and standard, industrial
strength technology.  The Kafka broker will hold the data for a configurable time if it is not
consumed.


### Extensible, maintainable code and minimal system dependencies

Sonar should be written in clean way in a portable, efficient language and in such a way that it can
support at least a modest variety of Unix-like operating systems.

**Implementation:**

The code is written in portable Rust with the Linux dependencies and GPU dependencies isolated in
subsystems.  A cursory study has indicated that the system is readily ported to, say, FreeBSD, and
we now support four different GPUs on two different CPU architectures.

Efficiency was studied at length in the past and was found to be more than acceptable, with low CPU
and memory usage.  Sonar needs to build some data structures during sampling but they are quickly
discarded and it uses little memory in its quiescent state.

For the most part, Rust has been an OK choice; it is not a fabulous language for prototyping but
fits the current, stable code well.

While it might seem that the GPU shims, written in C, could have been avoided had Sonar not being
written in Rust, this is not really so: the shims manage dynamic loading and data cleanup, and the
code would have been present if perhaps not exactly in the same form.

### Testable

Sonar should have a good test suite and means of letting itself be tested in a variety of scenarios.

**Implementation:**

There's a decent suite of white-box tests using the Rust selftest framework as well as a suite of
black-box tests that run Sonar in various modes (one-shot and daemon) using a variety of debug
settings (in the daemon config file or as environment variables).  Parsers are tested with mock
inputs and real inputs on nodes that can supply them.  GPU layers are tested on nodes with
appropriate GPUs.


### Aware of supply chain security issues

Sonar should not take dependencies on external Rust crated without good reason, so as to reduce the
impact of supply chain attacks on the Rust ecosystem.  It should be possible to disable features
that are not needed at as particular site.  "A little copying is better than a little dependency."

**Discussion:**

For a while, we tried to avoid external dependencies except for the libc crate, and small bits of
code we needed from other crates were copied into Sonar when the license allowed it.  (This was
pretty helpful in reducing code size too.)  With the introduction of Kafka support in the form of
the rdkafka crate, which wraps the librdkafka library, this strategy became impossible: there were
too many dependencies.

As a consequence of the floodgates being open, some more dependencies have been taken (for message
queues, signal handling, logging, and base64 encoding) and we should probably take some more (for
command line parsing and maybe for time handling, though pulling in clap and chrono respectively is
really not appealing).

A site that does not need Kafka can turn that off.  The main outstanding issue at the moment is that
for a site that will always upload data via the Kafka HTTP proxy, we could disable the rdkafka
library and leave the HTTP upload support in place, but can't currently do so.
