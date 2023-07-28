[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)


# sonar

Tool to profile usage of HPC resources by regularly probing processes using
`ps`.

All it really does is to run
`ps -e --no-header -o pid,user:22,pcpu,pmem,size,comm`
under the hood, and then filters and groups
the output and prints it to stdout, comma-separated.

![image of a fish swarm](img/sonar-small.png)

Image: [Midjourney](https://midjourney.com/), [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/legalcode)


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
      --cpu-cutoff-percent <CPU_CUTOFF_PERCENT>  [default: 0.5]
      --mem-cutoff-percent <MEM_CUTOFF_PERCENT>  [default: 5]
  -h, --help                                     Print help
```

The code will list all processes that are above `--cpu-cutoff-percent` or
`--mem-cutoff-percent`.


Here is an example output:
```console
$ sonar ps

2023-07-27T18:29:17+02:00,somehost,8,user,,alacritty,3.7,214932
2023-07-27T18:29:17+02:00,somehost,8,user,,slack,2.4,1328412
2023-07-27T18:29:17+02:00,somehost,8,user,,X,0.8,173148
2023-07-27T18:29:17+02:00,somehost,8,user,,brave,15.5,7085968
2023-07-27T18:29:17+02:00,somehost,8,user,,.zoom,37.8,1722564
```

The columns are:
- time stamp
- hostname
- number of cores on this node
- user
- Slurm job ID (empty if not applicable)
- process
- CPU percentage (as they come out of `ps`)
- memory used in KiB


## Collect results with `sonar analyze` :construction:

This part is work in progress. Currently we only collect the data since we use
it also in [another tool](https://github.com/NordicHPC/jobgraph). The mapping files can be found in the [data](data)
folder.


## Authors

- [Radovan Bast](https://bast.fr)
- Mathias Bockwoldt
- Lars T. Hansen
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

The tool does **not** need root permissions.  It does not modify anything and
only writes to stdout.

On CPUs, the only external command called by `sonar ps` is `ps -e --no-header -o
pid,user:22,pcpu,pmem,size,comm` and the tool gives up and stops if the latter
subprocess does not return within 2 seconds to avoid a pile-up of processes.

(we need to update this documentation for GPUs)


## How we run sonar on a cluster

We let cron execute the following script every 5 minutes on every compute node:
```bash
#!/usr/bin/env bash

set -euf -o pipefail

sonar_directory=/cluster/shared/sonar/data

year=$(date +'%Y')
month=$(date +'%m')
day=$(date +'%d')

output_directory=${sonar_directory}/${year}/${month}/${day}

mkdir -p ${output_directory}

/cluster/bin/sonar ps >> ${output_directory}/${HOSTNAME}.csv
```

This produces ca. 10 MB data per day.


## Similar and related tools

- Reference implementation which serves as inspiration:
  <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)
- [Ganglia Monitoring System](http://ganglia.info/)
