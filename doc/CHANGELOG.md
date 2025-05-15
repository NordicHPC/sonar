# Sonar change log

## Changes in v0.13.x-devel (on `main`)

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
