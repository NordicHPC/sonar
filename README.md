[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)


# sonar

Tool to profile usage of HPC resources by regularly probing processes.

Sonar examines `/proc` and runs some diagnostic programs and filters and groups the output and
prints it to stdout as CSV text.  The output format is defined in detail below.  Sonar can also
probe the system and reports on its overall configuration.

![image of a fish swarm](img/sonar-small.png)

Image: [Midjourney](https://midjourney.com/), [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/legalcode)


## Subcommands

Sonar has two subcommands, `ps` and `sysinfo`.  Both collect information about the system and print
it on stdout.  `sonar ps` collects information about running processes.  `sonar sysinfo` collects
information about the configuration of the system itself - cores, memory, gpus.

```console
$ sonar

Usage: sonar <COMMAND>

Commands:
  ps       Take a snapshot of the currently running processes
  sysinfo  Extract system information
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```


## Versions and release procedures

### Version numbers

The following basic versioning rules are new with v0.8.0.

We use semantic versioning.  The major version is expected to remain at zero for the foreseeable
future, reflecting the experimental nature of Sonar.

The minor version is updated with changes that alter the output format deliberately: fields are
added, removed, or are given a new meaning (this has been avoided so far), or the record format
itself changes.  For example, v0.8.0 both added fields and stopped printing fields that are zero.

The bugfix version is updated for changes that do not alter the output format per se but that might
affect the output nevertheless, ie, most changes not covered by changes to the minor version number.


### Release branches, uplifts and backports

The following branching scheme is new with v0.12.x.

The `main` branch is used for development and has a version number of the form `M.N.O-PRE` where
"PRE" is some arbitrary string, eg "devel", "rc4".  Note that this version number form will also be
present in the output of `sonar ps`, to properly tag those data.  If clients are exposed to
prerelease `ps` data they must be prepared to deal with this.

For every freeze of the the minor release number, a new release branch is created in the repo with
the name `release_<major>_<minor>`, again we expect `<major>` to remain `0` for the foreseeable
future, ergo, `release_0_12` is the v0.12.x release branch.  At branching time, the minor release
number is incremented on main (so when we created `release_0_12` for v0.12.1, the version number on
`main` went to `0.13.0-devel`).  The version number on a release branch is strictly of the form
M.N.O.

When a release `M.N.O` is to be made from a release branch, a tag is created of the form
`release_M_N_O` on that branch and the release is built from that changeset.  Once the release has
shipped, the bugfix version number on the branch is incremented.

With the branches come some additional rules for how to move patches around:

- If a bugfix is made to any release branch and the bug is present on main then the PR shall be
  tagged "uplift-required"; the PR shall subsequently be uplifted main; and following uplift the tag
  shall be changed to "uplifted-to-main".
- If a bugfix is made to main it shall be considered whether it should be backported the most recent
  release branch.  If so, the PR shall be tagged "backport-required"; the PR shall subsequently be
  cherry-picked or backported to the release branch; and following backport the tag shall be changed
  to "backported-to-release".  No older release branches shall automatically be considered for
  backports.


### Changes in v0.13.x-devel (on `main`)

Version in progress, no changes as of yet.


### Changes in v0.12.x (on `release_0_12`)

**System load data introduced**.  Added the `load` field which is printed with one of the records
per sonar invocation. (v0.12.0)


### Changes in v0.11.x

**Better `ps` data**.  More data points. (v0.11.0)


### Changes in v0.10.x

**Less output**.  Removed the `cores` and `memtotalkib` fields, as they are supplied by `sonar
sysinfo`. (v0.10.0)

**Batchless job ID**.  The meaning of the `job` field for systems without a batch queue (`sonar ps
--batchless`) has changed from being the pid of the process below the session leader to being the
more conventional process group id.  In most situations this won't make a difference. (v0.10.1)


### Changes in v0.9.x

**Sysinfo introduced**.  The `sonar sysinfo` subcommand was introduced to extract information about
the system itself.

**More help when information is missing**.  The user name field now includes the UID if the user
name can't be obtained from system databases but the UID is known. (v0.9.0)


### Changes in v0.8.x

**Better `ps` data**.  More clarifications, more data points. (v0.8.0)

**Less use of external programs**.  We go directly to `/proc` for data, and no longer run `ps`.

**Less `ps` output**. Fields that hold default values are not printed. (v0.8.0)


### Changes in v0.7.x

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


### Changes in v0.6.0

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


## Installation

- Make sure you have [Rust installed](https://www.rust-lang.org/learn/get-started) (I install Rust through `rustup`)
- Clone this project
- Build it: `cargo build --release`
- The binary is then located at `target/release/sonar`
- Copy it to where-ever it needs to be


## Collect processes with `sonar ps`

It's sensible to run `sonar ps` every 5 minutes on every compute node.

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

### Version 0.12.0 `ps` output format

Version 0.12.0 adds one field:

`load` (optional, default blank): This is an encoding of the per-cpu time usage in seconds on the
node since boot.  It is the same for all records and is therefore printed only with one of them per
sonar invocation.  The encoding is an array of N+1 u64 values for an N-cpu node.  The first value is
the "base" value, it is to be added to all the subsequent values.  The remaining are per-cpu values
in order from cpu0 through cpuN-1.  Each value is encoded little-endian base-45, with the initial
character of each value chosen from a different set than the subsequent characters.  The character
sets are:

```
INITIAL = "(){}[]<>+-abcdefghijklmnopqrstuvwxyz!@#$%^&*_"
SUBSEQUENT = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ~|';:.?/`"
```

The base-45 digits of the value `897` are (in little-endian order) 42 and 19, and the encoding of
this value is thus `&J`.  As the initial character is from a different character set, no explicit
separator is needed in the array - the initial digit acts as a separator.


### Version 0.11.0 `ps` output format

Version 0.11.0 adds one field:

`ppid` (optional, default "0"): The parent process ID of the job, a positive integer.


### Version 0.10.0 `ps` output format

The fields `cores` and `memtotalkib` were removed, as they were unused by all clients and are
supplied by `sonar sysinfo` for clients that need that information.


### Version 0.9.0 `ps` output format

Version 0.9.0 documents that the `user` field *in previous versions* could have the value
`_noinfo_`.  This value is sometimes observed in the output from older versions (though no clients
were looking for it).

Version 0.9.0 extends the encoding of the `user` field: it can now (also) have the value
`_noinfo_<uid>` where `<uid>` is the user ID, if user information was unobtainable for any reason
but we have a UID.  Clients could be able to handle both this encoding and the older encoding.


### Version 0.8.0 `ps` output format

Fields with default values (zero in most cases, or the empty set of GPUs) are not printed.

Version 0.8.0 adds two fields:

`memtotalkib` (optional, default "0"): The amount of physical RAM on this host, a nonnegative
integer, with 0 meaning "unknown".

`rssanonkib` (optional, default "0"): The current CPU data "RssAnon" (resident private) memory in KiB,
a nonnegative integer, with 0 meaning "no data available".

Version 0.8.0 also clarifies that the existing `cpukib` field reports virtual data+stack memory, not
resident memory nor virtual total memory.


### Version 0.7.0 `ps` output format

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
can also be `_zombie_<pid>` for zombie processes, where `<pid>` is the process ID of the process but
the user ID could not be obtained, or `_noinfo_<uid>`, where `<uid>` is the user ID of the process
but the user name could not be obtained.

`cmd` (required): The executable name of the process/command without command line arguments, an
alphanumeric string.  This can be `_unknown_` for zombie jobs, or `_noinfo_` for non-zombies when
the command name can't be found.

`cores` (optional, default "0", removed in v0.10): The number of cores on this host, a nonnegative
integer, with 0 meaning "unknown".

`memtotalkib` (optional, default "0", removed in v0.10): The amount of physical RAM on this host, a
nonnegative integer, with 0 meaning "unknown".

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

`ppid` (optional, default "0"): The parent process ID of the job, a positive integer.

`cpu%` (optional, default "0"): The running average CPU percentage over the true lifetime of the
process (ie computed independently of the sonar log), a nonnegative floating-point number.  100.0
corresponds to "one full core's worth of computation".

`cpukib` (optional, default "0"): The current CPU data+stack virtual memory used in KiB, a
nonnegative integer.

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


### Version 0.6.0 `ps` output format (and earlier)

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


## Collect system information with `sonar sysinfo`

The `sysinfo` subcommand collects information about the system and prints it in JSON form on stdout:

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

The `sysinfo` subcommand currently has no options.


### Version 0.9.0 `sysinfo` format

The JSON structure has these fields:

- `timestamp` - string, an ISO-format timestamp for when the information was collected
- `hostname` - string, the FQDN of the host
- `description` - string, a summary of the system configuration with model numbers and so on
- `cpu_cores` - number, the total number of virtual cores (sockets x cores-per-socket x threads-per-core)
- `mem_gb` - number, the amount of installed memory in GiB (2^30 bytes)
- `gpu_cards` - number, the number of installed accelerator cards
- `gpumem_gb` - number, the total amount of installed accelerator memory across all cards in GiB

Numeric fields that are zero may or may not be omitted by the producer.

Note the v0.9.0 `sysinfo` output does not carry a version number.


## Collect and analyze results

Sonar data are used by two other tools:

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


## Later design goals and design decisions

The needs of [Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and some bug fixes have led to
some feature creep (more data are reported), a bit of redesign (go directly to `/proc`, do not run
`ps`), and some quirky semantics (`cpu%` is only a good number for the first data point but is still
always reported, and `cputime/sec` is reported to complement it; and there's a distinction between
virtual and real memory that is possibly more useful on GPU-full and interactive systems than on HPC
CPU-only compute nodes).


## Security and robustness

The tool does **not** need root permissions.  It does not modify anything and writes output to
stdout (and errors to stderr).

On CPUs, no external commands are called by `sonar ps`.

On GPUs, `sonar ps` will attempt to use `nvidia-smi` and `rocm-smi` to record GPU utilization; the
tool gives up and stops if the latter subprocesses do not return within a few seconds to avoid a pile-up
of processes.

Optionally, `sonar` will use a lockfile to avoid a pile-up of processes.


## Dependencies and updates

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

This produces ca. 25-50 MB data per day on Saga (using mostly the old v0.5.0 output format), 5-20 MB
on Fox (including login and interactive nodes), using the new v0.8.0 output format), and 10-20MB per
day on the UiO ML nodes (all interactive), with significant variation.  Being text data, it
compresses extremely well.


## Similar and related tools

- Reference implementation which serves as inspiration:
  <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)
- [Ganglia Monitoring System](http://ganglia.info/)
