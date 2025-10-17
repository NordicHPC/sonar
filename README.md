[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)

# sonar

Sonar is a tool to profile usage of HPC resources by regularly sampling processes, accelerators,
nodes, queues, and clusters.

Sonar examines `/proc` and `/sys` and/or runs some diagnostic programs, filters and groups the
information, and prints it to stdout or sends it to a remote collector (notably via Kafka).

![image of a fish swarm](img/sonar-small.png)

Image: [Midjourney](https://midjourney.com/), [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/legalcode)

For more about the motivation, design, requirements, and other considerations, see [doc/DESIGN.md](doc/DESIGN.md).

## Subcommands

Sonar has several subcommands that collect information about nodes, jobs, clusters, and processes
and print it on stdout:

- `sonar ps` takes a snapshot of the currently running processes on the node and the node itself
- `sonar sysinfo` extracts hardware information about the node
- `sonar slurm` extracts information about overall job state from the slurm databases
- `sonar cluster` extracts information about partitions and node state from the slurm databases

Those subcommands are all run-once: Sonar exits after producing output.

Additionally, `sonar daemon` starts Sonar and keeps it memory-resident, running subprograms at
intervals specified by a configuration file.  In the daemon mode, exfiltration of data is to a
remote Kafka broker or into a directory tree, also specified in the configuration file.

Finally, `sonar help` prints some useful help and `sonar version` prints the version number.

## Compilation and installation

In principle you just do this:

- Make sure you have [Rust installed](https://www.rust-lang.org/learn/get-started) (I install Rust through `rustup`)
- Clone this project
- If building with Kafka support (the default), you must have the OpenSSL development libraries installed,
  [as noted here](https://docs.rs/rdkafka/0.37.0/rdkafka/#installation).
  On Ubuntu, this is libssl-dev, on Fedora it is openssl-devel.
- Build it: `cargo build --release`
- The binary is then located at `target/release/sonar`
- Copy it to wherever it needs to be

In practice it is a little harder:

- The binutils you have need to be new enough for the assembler to understand `--gdwarf5`
  (for Kafka) and some other things (to link the GPU probe libraries)
- Some of the tests in `util/` (if you are going to be running those) require `go`

Some distros, notably RHEL8, have binutils that are too old, you can check by running e.g.
`as --version`, the major version number is also the version number of binutils.  Binutils 2.32
are new enough for the GPU probe libraries but may not be new enough for Kafka.  Binutils 2.40
are known to work for both.  Also see comments in `gpuapi/Makefile`.

## Output format options

There are two output formats, [the old format](doc/OLD-FORMAT.md) and [the new
format](doc/NEW-FORMAT.md), currently coexisting but the old format will be phased out.

The recommended (and default as of v0.16) output format is the "new" JSON format.  There are command
line switches to force the older formats, CSV or an older JSON format.

## Examples

Some illustrative runs.  For more detailed instructions on how to use it, see "How we run sonar on a
cluster", below.  For a full description of the output formats and fields, see the previous section.

### Collect processes with `sonar ps`

It's sensible to run `sonar ps` every 5 minutes on every compute node if you care mostly about
long-running jobs, or at higher frequency if sbrief jobs are of interest to you.

Here is an example output (with the older CSV output format):
```console
$ sonar ps --exclude-system-jobs --min-cpu-time=10

v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=fish,cpu%=2.1,cpukib=64400,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=138
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=sonar,cpu%=761,cpukib=372,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=137
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=brave,cpu%=14.6,cpukib=2907168,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=3532
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=alacritty,cpu%=0.8,cpukib=126700,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=51
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=pulseaudio,cpu%=0.7,cpukib=90640,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=399
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=slack,cpu%=3.9,cpukib=716924,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=266
```

### Collect system information with `sonar sysinfo`

The `sysinfo` subcommand collects information about the system and prints it in JSON form on stdout
(this is the older JSON format):

```console
$ sonar sysinfo
{
 "timestamp": "2024-02-26T00:00:02+01:00",
 "hostname": "ml1.hpc.uio.no",
 "description": "2x14 (hyperthreaded) Intel(R) Xeon(R) Gold 5120 CPU @ 2.20GHz, 125 GB, 3x NVIDIA GeForce RTX 2080 Ti @ 11GB",
 "cpu_cores": 56,
 "mem_gb": 125,
 "gpu_cards": 3,
 "gpumem_gb": 33
}
```

Typical usage for `sysinfo` is to run the command after reboot and (for hot-swappable systems and
VMs) once every 24 hours, and to aggregate the information in some database.

### Collecting job information with `sonar slurm`

The `slurm` command runs `sacct` and extracts job data.  This command exists partly to allow
clusters to always push data, partly to collect the data for long-term storage, partly to offload
the Slurm database manager during query processing.

```console
$ sonar slurm --deluge --json --cluster my.cluster
...
```

The `--deluge` option extracts running and pending jobs as well as completed jobs.

### Collecting partition and node information with `sonar cluster`

The `cluster` command runs `sinfo` and extracts cluster (partition) information and node
information.  This command exists partly to allow clusters to always push data, partly to collect
the data for long-term storage.

```console
$ sonar cluster --cluster my.cluster
...
```

The output is always JSON.

## Collect and analyze results

Sonar data are used by two other tools:

* [JobAnalyzer](https://github.com/NAICNO/Jobanalyzer) allows Sonar logs to be queried and analyzed, and
  provides dashboards, interactive and batch queries, and reporting of system activity, policy violations,
  hung jobs, and more.  It is under active development.
* [JobGraph](https://github.com/NordicHPC/jobgraph) provides high-level plots of system activity. Mapping
  files for JobGraph can be found in the [data](data) folder.  Its development has been dormant for some
  time.

## Versions and release procedures

We use semantic versioning.  The major version is expected to remain at zero for the foreseeable
future, reflecting the experimental nature of Sonar.

At the time of writing we require:
- 2021 edition of Rust
- Rust 1.77.2 (can be found with `cargo msrv find`)

For all other versioning information, see [doc/VERSIONING.md](doc/VERSIONING.md).

## Authors

- [Radovan Bast](https://bast.fr)
- Mathias Bockwoldt
- [Lars T. Hansen](https://github.com/lars-t-hansen)
- Henrik Rojas Nagel

## How we run sonar on a cluster

See [doc/HOWTO-DEPLOY.md](doc/HOWTO-DEPLOY.md).

## Similar and related tools

Sonar's original vision was as a very simple, lightweight tool that did some basic things fairly
cheaply and produced easy-to-process output for subsequent scripting.  Sonar is no longer that: with
GPU integration, SLURM integration, Kafka exfiltration, memory-resident modes, structured output,
continual focus on performance and elaborate backends in
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and
[Slurm-monitor](https://github.com/2maz/slurm-monitor), it is becoming as complex as the tools it
was intended to replace or compete with.

Here are some of those tools:

- [Trailblazing Turtle](https://github.com/guilbaults/TrailblazingTurtle), SLURM-specific but similar to Sonar.
- [Scaphandre](https://hubblo-org.github.io/scaphandre-documentation/index.html), for energy monitoring.
- [Sysstat and SAR](https://github.com/sysstat/sysstat), for monitoring a lot of things.
- [seff](https://support.schedmd.com/show_bug.cgi?id=1611), SLURM-specific.
- [TACC Remora](https://github.com/tacc/remora)
- Reference implementation which serves as inspiration:
  <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)
- [Ganglia Monitoring System](http://ganglia.info/)
