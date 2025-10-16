# Sonar change log

Older sections are at best partial.  Newer sections (since 0.15) aim to be comprehensive.

Since v0.13.0, the output format is documented mainly by the file
[util/formats/newfmt/types.go](../util/formats/newfmt/types.go), which specifies the new JSON output
format in terms of a Go data structure with JSON annotations.  Changes to the output format can most
easily be be seen by diffing this file against a desired tag, starting with "v0.13.0".  Such changes
are therefore no longer mentioned specially below.  Recall that to get the new format, Sonar must be
run in daemon mode or with the --json switch (which also requires the --cluster).

The "old" output formats are for the most part not being extended with new fields and should not be
relied upon.  Consumers should expect to see the "new" JSON format for new data, and all producers
should ask for that format.

## Changes in v0.16.0 (on `release_0_16`)

* Bug 335 - **BREAKING CHANGE.** The `cpu_util` field was not emitted properly, it should have been
  scaled by 100 according to docs (it's a percentage).  Older data will look weird compared to new
  data
* Bug 352 - **MAJOR FUNCTIONALITY.**  Populate the `topo_svg` field, introduce a `topo_text` field,
  and expand the config file so as to be able to specify how to generate those data
* Bug 353 - Remove the `software` field.
* Bug 387 - **IMPORTANT FIX.** Parse some slurm data, notably ReqMem, correctly.  Previously the parser
  would always return 0 for this field
* Bug 401 - better error messages for Kafka
* Bug 402 - synthesize a UUID on AMD cards when needed
* Bug 402 - change the syntax of synthesized UUIDs on XPU (now joint with AMD)
* Bug 403 - allow cards from multiple manufacturers on the same node
* Bug 407 - clarify semantics of the GPU index field in several data types
* Testing (sundry): Many testing improvements
* Doc (sundry): Various improvements


## Changes in v0.15.0 (on `release_0_15`)

* Feature (Rust/C): Support for XPU and Habana GPUs
* Cleanup (Rust): Signal handling improvements
* Cleanup (Rust): Rewrite daemon and Kafka message pumps
* Testing (sundry): Many testing improvements
* Rust: Dependency updates
* Rust: Minimum Rust version is now 1.77


## Changes in v0.14.x-devel (on `release_0_14`)

**New output sink**: A 'directory' output sink for the daemon mode.  (v0.14.0)

**Cleanup**: Documentation.  Code reorg and cleanup, abstracting out Linux support.  (v0.14.0)


## Changes in v0.13.x (on `release_0_13`)

**Bug fixes**.  Sundry daemon bug fixes (v0.13.1 and v0.13.2)

**Kafka exfiltration**.  Data can be sent via the Kafka protocol (with SSL encryption and SASL
authentication) to a network broker, rather than relying on an external agents for communication.
(v0.13.0)

**Daemon mode**.  In this mode, Sonar will stay memory-resident and perform sampling and information
gathering according to a configuration file, normally exfiltrating the data over a network channel
using built-in exfiltration.  This aims to reduce the overhead of running Sonar.  (v0.13.0)

**New JSON format**.  The new format is obtained using `--json` for all command and is a clean
format described in prose in [doc/NEW-FORMAT.md](NEW-FORMAT.md) and in executable form in
[util/formats/newfmt/types.go](../util/formats/newfmt/types.go).  (v0.13.0)

**`sonar cluster` command introduced**.  For clusters under Slurm control, this will produce data about
the cluster configuration (partitions, nodes).  (v0.13.0)

**Many new data fields in the new data format**.  Too many to summarize here.  (v0.13.0)

**All commands can emit JSON or CSV under a command-line switch**.  The defaults remain the same for now:
CSV for `ps` and `slurm`, JSON for `sysinfo`.  (v0.13.0)

**Per-GPU load data introduced**.  Added the `gpuinfo` field which is printed with one of the records
per `sonar ps` invocation. (v0.13.0)

**`sonar slurm` command introduced**.  This extracts information from the Slurm database about
completed jobs within a time window, on CSV format.  (v0.13.0)

**Use SMI libraries**.  Sonar will no longer run `nvidia-smi` and `rocm-smi` to obtain GPU data but
will dynamically load the cards' SMI libraries and obtain data via them.  (v0.13.0)


## Changes in v0.12.x (on `release_0_12`)

**System load data introduced**.  Added the `load` field which is printed with one of the records
per `sonar ps` invocation. (v0.12.0)


## Changes in v0.11.x

**Better `ps` data**.  More data points. (v0.11.0)


## Changes in v0.10.x

**Less output**.  Removed the `cores` and `memtotalkib` fields, as they are supplied by `sonar
sysinfo`. (v0.10.0)

**Batchless job ID**.  The meaning of the `job` field for systems without a batch queue (`sonar ps
--batchless`) has changed from being the pid of the process below the session leader to being the
more conventional process group id.  In most situations this won't make a difference. (v0.10.1)


## Changes in v0.9.x

**Sysinfo introduced**.  The `sonar sysinfo` subcommand was introduced to extract information about
the system itself.

**More help when information is missing**.  The user name field now includes the UID if the user
name can't be obtained from system databases but the UID is known. (v0.9.0)


## Changes in v0.8.x

**Better `ps` data**.  More clarifications, more data points. (v0.8.0)

**Less use of external programs**.  We go directly to `/proc` for data, and no longer run `ps`.

**Less `ps` output**. Fields that hold default values are not printed. (v0.8.0)


## Changes in v0.7.x

**Improved `ps` process filtering.** The filters used in previous versions (minimum CPU
`--min-cpu-percent`, and memory usage `--min-mem-percent`) are nonmonotonic in that records for a
long-running job can come and go in the log over time.  Those filters are still available but
monotonic filters (for non-system jobs `--exclude-system-jobs`, and for jobs that have been running
long enough `--min-cpu-time`) are now available and will result in more easily understood data.

**Improved `ps` process merging.** Earlier, sonar would merge some processes unconditionally and
somewhat opaquely.  All merging is now controlled by command-line switches and is more transparent.

**Better `ps` data.** Additional data points are gathered, notably for GPUs, and the meaning of the data
being gathered has been clarified.

**Self-documenting `ps` data.** The output format has changed to use named fields, which allows the introduction
of fields and the use of default values.

**Clearer division of labor with a front-end tool.** Front-end tools such as
[jobgraph](https://github.com/NordicHPC/jobgraph) and
[sonalyze](https://github.com/NAICNO/Jobanalyzer/tree/main/code/sonalyze) ingest well-defined and
simply-created sonar data, process it and present it in specialized ways, removing those burdens
from sonar.


## Changes in v0.6.0

**This tool focuses on how resources are used**. What is actually running.  Its
focus is not (anymore) whether and how resources are under-used compared to
Slurm allocations. But this functionality can be re-inserted.

**We have rewritten it from Python to Rust**. The motivation was to have one
self-contained binary, without any other dependencies or environments to load,
so that the call can execute in milliseconds and so that it has minimal impact
on the resources on a large computing cluster. You can find the Python version
on the [python](https://github.com/NordicHPC/sonar/tree/python) branch.

Versions through 0.5.0 are available on [PyPI](https://pypi.org/project/sonar/).

You can find the old code on the
[with-slurm-data](https://github.com/NordicHPC/sonar/tree/with-slurm-data)
branch.
