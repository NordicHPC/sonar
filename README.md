[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)


# sonar

Tool to profile usage of HPC resources by regularly probing processes using `ps` and other tools.

All it really does is to run `ps` and other diagnostic programs under the hood, and then filters and
groups the output and prints it to stdout, comma-separated.  The file format is defined in detail
below.

![image of a fish swarm](img/sonar-small.png)

Image: [Midjourney](https://midjourney.com/), [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/legalcode)


## Changes since v0.6.0

**Improved filtering.** The process filters used in previous versions (minimum CPU and memory usage)
are nonmonotonic in that job records for a long-running job can come and go in the log over time.
Those filters are still available but monotonic filters (for non-system jobs and for jobs that have
been running long enough) are now available and will result in more easily understood jobs.

**Improved merging.** Earlier, sonar would merge some processes unconditionally and somewhat
opaquely.  All merging is now controlled by command-line switches and more transparent.

**Better data.** Additional data points are gathered, notably for GPUs, and the meaning of the data
being gathered has been clarified.

**Self-documenting.** The file format has changed to use named fields, which allows the introduction
of fields and the use of default values.

**Clearer division of labor with a front-end tool.** Front-end tools such as
[jobgraph](https://github.com/NordicHPC/jobgraph) and
[sonalyze](https://github.com/NAICNO/Jobanalyzer/tree/main/sonalyze) ingest well-defined and
simply-created sonar data, process it and present it in specialized ways, removing those burdens
from sonar.

## Changes since v0.5.0

**This tool focuses on how resources are used**. What is actually running.  Its
focus is not (anymore) whether and how resources are under-used compared to
Slurm allocations. But this functionality can be re-inserted.

**We have rewritten it from Python to Rust**. The motivation was to have one
self-contained binary, without any other dependencies or environments to load,
so that the call can execute in milliseconds and so that it has minimal impact
on the resources on a large computing cluster. You can find the Python version
on the [python](https://github.com/NordicHPC/sonar/tree/python) branch.

Versions until 0.5.0 are available on [PyPI](https://pypi.org/project/sonar/).

You can find the old code on the
[with-slurm-data](https://github.com/NordicHPC/sonar/tree/with-slurm-data)
branch.


## Installation

- Make sure you have [Rust installed](https://www.rust-lang.org/learn/get-started) (I install Rust through `rustup`)
- Clone this project
- Build it: `cargo build --release`
- The binary is then located at `target/release/sonar`
- Copy it to where-ever it needs to be


## Collect processes with `sonar ps`

Available options:
```console
$ sonar

Usage: sonar <COMMAND>

Commands:
  ps       Take a snapshot of the currently running processes
  analyze  Not yet implemented
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

We run `sonar ps` every 5 minutes on every compute node.

```console
$ sonar ps --help
Take a snapshot of the currently running processes

Usage: sonar ps [OPTIONS]

Options:
      --batchless
          Synthesize a job ID from the process tree in which a process finds itself
      --rollup
          Merge process records that have the same job ID and command name
      --min-cpu-percent <MIN_CPU_PERCENT>
          Include records for jobs that have on average used at least this percentage of CPU, note this is nonmonotonic [default: none]
      --min-mem-percent <MIN_MEM_PERCENT>
          Include records for jobs that presently use at least this percentage of real memory, note this is nonmonotonic [default: none]
      --min-cpu-time <MIN_CPU_TIME>
          Include records for jobs that have used at least this much CPU time (in seconds) [default: none]
      --exclude-system-jobs
          Exclude records for system jobs (uid < 1000)
      --exclude-users <EXCLUDE_USERS>
          Exclude records for these comma-separated user names [default: none]
      --exclude-commands <EXCLUDE_COMMANDS>
          Exclude records whose commands start with these comma-separated names [default: none]
      --lockdir <LOCKDIR>
          Create a per-host lockfile in this directory and exit early if the file exists on startup [default: none]
  -h, --help
          Print help
```

**NOTE** that if you use `--lockdir`, it should name a directory that is cleaned on reboot, such as
`/var/run`, `/run`, or a tmpfs, and ideally it is a directory on a disk local to the node, not a
shared disk.

Here is an example output:
```console
$ sonar ps --exclude-system-jobs --min-cpu-time=10 --rollup

v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=fish,cpu%=2.1,cpukib=64400,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=138
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=sonar,cpu%=761,cpukib=372,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=137
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=brave,cpu%=14.6,cpukib=2907168,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=3532
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=alacritty,cpu%=0.8,cpukib=126700,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=51
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=pulseaudio,cpu%=0.7,cpukib=90640,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=399
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=slack,cpu%=3.9,cpukib=716924,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=266
```

### Version 0.8.0 file format (evolving)

Fields with default values are not printed.

### Version 0.7.0 file format

Each field has the syntax `name=value` where the names are defined below.  Fields are separated by
commas, and each record is terminated by a newline.  The syntax of the file is therefore as for CSV
(including all rules for quoting).  However the semantics do not adhere to strict CSV: there may be
a variable number of fields ("columns"), and as the fields are named, they need not be presented in
any particular order.  Not all of the fields may be present - they have default values as noted
below.  Consumers should assume that new fields may appear, and should not treat records with
unknown field names as errors.  Broadly we would like to guarantee that fields never change meaning.

Integer fields will tend to be truncated toward zero, not rounded or rounded up.

The field names and their meaning are:

`v` (required): The record version number, a semantic version number on the format `n.m.o`.

`time` (required): The time stamp of the sample, an ISO time format string without fractional
seconds but with TZO.  Every record created from a single invocation of `sonar` has the same
timestamp (consumers may depend on this).

`host` (required): The fully qualified domain name of the host running the job, an alphanumeric
string.  There is only a single host.  If the job spans hosts, there will be multiple records for
the job, one per host; see `job` below.

`user` (required): The local Unix user name of user owning the job, an alphanumeric string.  This
can also be `_zombie_<pid>` for zombie processes, where `<pid>` is the process ID of the process.

`cmd` (required): The executable name of the process/command without command line arguments, an
alphanumeric string.  This can be `_unknown_` for zombie jobs, or `_noinfo_` for non-zombies when
the command name can't be found.

`cores` (optional, default "0"): The number of cores on this host, a nonnegative integer, with 0
meaning "unknown".

`memtotalkib` (optional, default "0"): The amount of physical RAM on this host, a nonnegative
integer, with 0 meaning "unknown".

`job` (optional, default "0"): The job ID, a positive integer. This field will be 0 if the job or
process does not have a meaningful ID.  There may be many records for the same job, one for each
process in the job (subject to filtering); these records can have different host names too.
Processes in the same job on the same host are merged if the `--rollup` command line option is used
and the processes have the same `cmd` value.

NOTE CAREFULLY that if the job ID is 0 then the process record is for a unique job with unknown job
ID.  Multiple records with the job ID 0 should never be merged into a single job by the consumer.

`pid` (optional, default "0"): The process ID of the job, a positive integer.  For a rolled-up job
(see `rolledup` below) this has value zero.  Otherwise, this record represents one process and so
the field holds the process ID.

`cpu%` (optional, default "0"): The running average CPU percentage over the true lifetime of the
process (ie computed independently of the sonar log), a nonnegative floating-point number.  100.0
corresponds to "one full core's worth of computation".

`cpukib` (optional, default "0"): The current CPU data virtual memory used in KiB, a nonnegative
integer.

`rssanonkib` (optional, default "0"): The current CPU data "RssAnon" (resident private) memory in KiB,
a nonnegative integer, with 0 meaning "no data available".

`gpus` (optional, default "none"): The list of GPUs currently used by the job, a comma-separated
list of GPU device numbers, all of them nonnegative integers.  The value can instead be `none` when
the process uses no GPUs, or `unknown` when the process is known to use GPUs but their device
numbers can't be determined.

`gpu%` (optional, default "0"): The current GPU percentage utilization summed across all cards, a
nonnegative floating-point number.  100.0 corresponds to "one full card's worth of computation".

`gpukib` (optional, default "0"): The current GPU memory used in KiB, a nonnegative integer.  This
is summed across all cards.

The difference between `gpukib` and `gpumem%` (below) is that, on some cards some of the time, it is
possible to determine one of these but not the other, and vice versa.  For example, on the NVIDIA
cards we can read both quantities for running processes but only `gpukib` for some zombies.  On the
other hand, on our AMD cards there is no support for detecting the absolute amount of memory used,
nor the total amount of memory on the cards, only the percentage of gpu memory used.  Sometimes we
can convert one figure to another, but other times we cannot quite do that.  Rather than encoding
the logic for dealing with this in sonar, the task is currently offloaded to the front end.

`gpumem%` (optional, default "0"): The current GPU memory usage percentage, a nonnegative
floating-point number.  This is summed across all cards.  100.0 corresponds to "one full card's
worth of memory".

`cputime_sec` (optional, default "0"): Accumulated CPU time in seconds that a process has used over
its lifetime, a nonnegative integer.  The value includes time used by child processes that have
since terminated.

`rolledup` (optional, default "0"): The number of additional processes with the same `job` and `cmd`
that have been rolled into this one in response to the `--rollup` switch.  That is, if the value is
`1`, the record represents the sum of the data for two processes.  If a record represents part of a
rolled-up job then this field must be present.


### Version 0.6.0 file format (and earlier)

The fields in version 0.6.0 are unnamed and the fields are always presented in the same order.  The
fields have (mostly) the same syntax and semantics as the 0.7.0 fields, with these notable differences:

* The time field has a fractional-second part and is always UTC (the TZO is always +00:00)
* The `gpus` field is a base-2 binary number representing a bit vector for the cards used; for the `unknown` value, it is a string of `1` of length 32.

The order of fields is:

`time`, `host`, `cores`, `user`, `job`, `cmd`, `cpu%`, `cpukib`, `gpus`, `gpu%`, `gpumem%`, `gpukib`

where the fields starting with `gpus` may be absent and should be taken to have the defaults
presented above.

Earlier versions of `sonar` would always roll up processes with the same `job` and `cmd`, so older
records may or may not represent multiple processes' worth of data.


## Collect results with `sonar analyze` :construction:

The `analyze` command is work in progress.  Sonar data are used by two other tools:

* [JobGraph](https://github.com/NordicHPC/jobgraph) provides high-level plots of system activity. Mapping
  files for JobGraph can be found in the [data](data) folder.
* [JobAnalyzer](https://github.com/NAICNO/Jobanalyzer) allows sonar logs to be queried and analyzed, and
  provides dashboards, interactive and batch queries, and reporting of system activity, policy violations,
  hung jobs, and more.

## Authors

- [Radovan Bast](https://bast.fr)
- Mathias Bockwoldt
- [Lars T. Hansen](https://github.com/lars-t-hansen)
- Henrik Rojas Nagel


## Design goals and design decisions

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

On CPUs, the only external command called by `sonar ps` is `ps`; the tool gives up and stops if the
latter subprocess does not return within 2 seconds to avoid a pile-up of processes.

On GPUs, `sonar ps` will attempt to use `nvidia-smi` and `rocm-smi` to record GPU utilization.


## How we run sonar on a cluster

We let cron execute the following script every 5 minutes on every compute node:
```bash
#!/usr/bin/env bash

set -euf -o pipefail

sonar_directory=/cluster/shared/sonar/data

path=$(date '+%Y/%m/%d')
output_directory=${sonar_directory}/${path}

mkdir -p ${output_directory}

/cluster/bin/sonar ps >> ${output_directory}/${HOSTNAME}.csv
```

This produces ca. 10 MB data per day.


## Similar and related tools

- Reference implementation which serves as inspiration:
  <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)
- [Ganglia Monitoring System](http://ganglia.info/)
