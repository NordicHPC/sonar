[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)


# sonar

Tool to profile usage of HPC resources by regularly probing processes.

Sonar examines `/proc` and runs some diagnostic programs and filters and groups the output and
prints it to stdout.  There are two output formats, [the old format](doc/OLD-FORMAT.md) and [the new
format](doc/NEW-FORMAT.md), currently coexisting but the old format will be phased out.  Sonar can
also probe the system and reports on its overall configuration.

![image of a fish swarm](img/sonar-small.png)

Image: [Midjourney](https://midjourney.com/), [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/legalcode)


## Subcommands

Sonar has several subcommands that collect information about nodes, jobs, clusters, and processes
and print it on stdout:

- `sonar ps` takes a snapshot of the currently running processes
- `sonar sysinfo` extracts hardware information about the node
- `sonar slurm` extracts information about overall job state from the slurm databases
- `sonar cluster` extracts information about partitions and node state from the slurm databases
- `sonar help` prints some useful help


## Compilation and installation

- Make sure you have [Rust installed](https://www.rust-lang.org/learn/get-started) (I install Rust through `rustup`)
- Clone this project
- Build it: `cargo build --release`
- The binary is then located at `target/release/sonar`
- Copy it to wherever it needs to be

If the build results in a link error for `libsonar-<something>.a` then your binutils are too old,
this can be a problem on eg RHEL9.  See comments in `gpuapi/Makefile` for how to resolve this.


## Output format options

The recommended output format is the "new" JSON format.  Use the command line switch `--json` with
all commands to force this format.  Most subcommands currently default to either CSV or an older
JSON format.


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

Here is an example output (with the default CSV output format):
```console
$ sonar ps --exclude-system-jobs --min-cpu-time=10 --rollup

v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=fish,cpu%=2.1,cpukib=64400,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=138
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=sonar,cpu%=761,cpukib=372,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=137
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=brave,cpu%=14.6,cpukib=2907168,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=3532
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=alacritty,cpu%=0.8,cpukib=126700,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=51
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=pulseaudio,cpu%=0.7,cpukib=90640,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=399
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=slack,cpu%=3.9,cpukib=716924,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=266
```

## Collect system information with `sonar sysinfo`

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

The `sysinfo` subcommand currently has no options.


## Collecting job information with `sonar slurm`

To be written.

This command exists partly to allow clusters to always push data, partly to collect the data for
long-term storage, partly to offload the Slurm database manager during query processing.

## Collecting partition and node information with `sonar cluster`

To be written.

This command exists partly to allow clusters to always push data, partly to collect the data for
long-term storage.

## Collect and analyze results

Sonar data are used by two other tools:

* [JobGraph](https://github.com/NordicHPC/jobgraph) provides high-level plots of system activity. Mapping
  files for JobGraph can be found in the [data](data) folder.
* [JobAnalyzer](https://github.com/NAICNO/Jobanalyzer) allows sonar logs to be queried and analyzed, and
  provides dashboards, interactive and batch queries, and reporting of system activity, policy violations,
  hung jobs, and more.


## Output formats

See [doc/OLD-FORMAT.md](doc/OLD-FORMAT.md) and [doc/NEW-FORMAT.md](doc/NEW-FORMAT.md) for
specifications of the output data formats and the semantics of individual fields.


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


### Policies for changing Rust edition and minimum Rust version

At the time of writing we require:
- 2021 edition of Rust
- Rust 1.59.0, released 2022-02-24 (can be found with `cargo msrv find`)

Policy for changing the minimum Rust version:
- Open a GitHub issue and motivate the change
- Once we reach agreement in the issue discussion:
  - Update the version inside the test workflow [test-minimal.yml](.github/workflows/test-minimal.yml)
  - Update the documentation (this section)


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

No external commands are called by `sonar ps` or `sonar sysinfo`: Sonar reads `/proc` and probes the
GPUs via their manufacturers' SMI libraries to collect all data.

The Slurm `sacct` command is currently run by `sonar slurm`.  A timeout mechanism is in place to
prevent this command from hanging indefinitely.

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
