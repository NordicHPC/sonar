# Design, security, etc

## Current (2025Q4) high-level requirements

These are sometimes post-hoc but they are the requirements guiding the work right now:

- Good enough data to expose interesting behavior
- Minimal overhead for recording
- Minimal overhead for data exfiltration
- Extensible and well-defined output format
- Extensible, maintainable code and minimal system dependencies
- Support for non-SLURM (indeed, non-batch-job) systems
- Support for containers
- Robust / fault-tolerant
- Secure and not dependent on running as root
- Standard setup (eg as a systemd service)

These are mostly met as follows:

**Good enough data to expose interesting behavior:** This requirement is a little tricky since it is
defined by the consumer, primarily [Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and its
sibling project [Slurm-monitor](https://github.com/2maz/slurm-monitor), and their use cases, which
are themselves evolving.  Over time, Sonar has evolved from simple per-process-per-node sampling to
incorporate much more per-process data, per-node hardware and OS samples, node configuration data,
and companion cluster and jobs data from SLURM.  For example, in response to requests from users we
added per-CPU sampling to expose incorrect use of hyperthreading, and we added per-process thread
counts and per-node load averages which together with node configuration data and job allocation
data serve to expose imbalances in the node and jobs - a typical case is that a node is overloaded
with too many runnable threads or, equivalently, that a job is underprovisioned on compute.  More
data points will be added as they are needed by concrete use cases.

Another aspect of the data is their groundedness - they must be reliable as a view of the system
being monitored.  Data from one job must be properly separated from data from another job.  (This is
surprisingly involved on systems without a batch job system and no local Sonar state.)  The data
must furthermore be relevant and true and have well understood semantics.

**Minimal overhead for recording:** We go directly to /proc and /sys for most data, we load the
GPUs' SMI libraries for access to GPU data, and we avoid running external programs for process
sampling (which is the only performance-sensitive operation).

**Minimal overhead for data exfiltration:** The expected setup is to exfiltrate via Kafka over a
secure channel.  Kafka communications can be batched and held, and the channel can remain open
between the node and the broker between communications.  If the broker is on-cluster the channel
does not need to be encrypted (probably).  Our JSON data are large but are compressed in transit at
the moment.  Should we need higher efficiency we could move to protobuf for transmission.

**Extensible and well-defined output format:** The JSON data structure is formally defined in terms
of a Go data structure with json annotations for field names and doc comments for documentation.
This spec is machine-processable and we use it to generate corresponding data in several other
formats.

**Extensible, maintainable code and minimal system dependencies:** The code is written in portable,
idiomatic Rust with the Linux dependencies and GPU dependencies isolated in subsystems.  A cursory
study has indicated that the system is readily ported to, say, FreeBSD, and we now support four
different GPUs on two different CPU architectures.

**Support for non-SLURM and non-batch-job systems:** Mostly this means that a "job" concept has to
be layered on the underlying samples (this is easy), and that data analysis must be possible without
relying on batch system metadata.

**Support for containers:** A job that runs inside a container (often a non-batch-system job in that
case, or a container that's being run by the batch system) must be visible to Sonar and Sonar,
running outside the container, must be able to collect meaningful data about the job.

**Robust and fault-tolerant:** Sonar must have well-understood error behavior; errors that are not
in communication itself must be detectable on the consumer side; and communication errors must be
detectable on the node.

**Secure and generally not dependent on running as root:** We don't want to have to trust Sonar and
it needs to be possible for Sonar to collect most meaningful data without root access, running as a
normal user.

**Standard setup:** It must be possible to set up Sonar in a way that fits in well with the system
it is running on, ie, it needs to be possible to manage it as one-shot runs via cron, as a daemon
via systemd, and similar.

## Intermediate (ca 2023/2024) design goals and design decisions

Relative to the "early" goals (below), the needs of
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and some bug fixes led to some feature creep
(more data were reported), a bit of redesign (Sonar would go directly to `/proc`, do not run `ps`),
and some quirky semantics (`cpu%` is only a good number for the first data point but is still always
reported, and `cputime/sec` is reported to complement it; and there's a distinction between virtual
and real memory that is possibly more useful on GPU-full and interactive systems than on HPC
CPU-only compute nodes).

Other than that, the Intermediate goals were a mix of early goals and the current requirements,
above.

## Early design goals and design decisions

- Easy installation
- Minimal overhead for recording
- Can be used as health check tool
- Does not need root permissions

**Use `ps` instead of `top`**:
We started using `top` but it turned out that `top` is dependent on locale, so
it displays floats with comma instead of decimal point in many non-English
locales. `ps` always uses decimal points. In addition, `ps` is (arguably) more
versatile/configurable and does not print the header that `top` prints. All
these properties make the `ps` output easier to parse than the `top` output.

**Do not interact with the Slurm database at all**:
The initial version correlated information we gathered from `ps` (what is
actually running) with information from Slurm (what was requested). This was
useful and nice to have but became complicated to maintain since Slurm could
become unresponsive and then processes were piling up.

**Why not also recording the `pid`**?:
Because we sum over processes of the same name that may be running over many
cores to have less output so that we can keep logs in plain text
([csv](https://en.wikipedia.org/wiki/Comma-separated_values)) and don't have to
maintain a database or such.


## Security and robustness

The tool does **not** need root permissions.  It does not modify anything and writes output to
stdout (and errors to stderr).

No external commands are called by `sonar ps` or `sonar sysinfo`: Sonar reads `/proc` and probes the
GPUs via their manufacturers' SMI libraries to collect all data.

The Slurm `sacct` command is currently run by `sonar slurm` and `sinfo` is run by `sonar cluster`.
A timeout mechanism is in place to prevent these commands from hanging indefinitely.

Optionally, `sonar` will use a lockfile to avoid a pile-up of processes.


## Dependencies and updates

(This section is obsolete.  We gave up on supply chain security around v0.14 as the introduction of
the Kafka library required the introduction of a large number of crates we cannot trust.  Users who
don't need Kafka can remove it and likely will see the number of dependencies drop significantly.
Alas, leaning into this, we have since added more dependencies for multi-threading channels and
base64 encoding, which may themselves add dependencies.)

Sonar runs everywhere and all the time, and even though it currently runs without privileges it
strives to have as few dependencies as possible, so as not to become a target through a supply chain
attack.  There are some rules:

- It's OK to depend on libc and to incorporate new versions of libc
- It's better to depend on something from the rust-lang organization than on something else
- Every dependency needs to be justified
- Every dependency must have a compatible license
- Every dependency needs to be vetted as to active development, apparent quality, test cases
- Every dependency update - even for security issues - is to be considered a code change that needs review
- Remember that indirect dependencies are dependencies for us, too, and need to be treated the same way
- If in doubt: copy the parts we need, vet them thoroughly, and maintain them separately

There is a useful discussion of these matters [here](https://research.swtch.com/deps).
