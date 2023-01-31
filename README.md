[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)
[![image](https://badge.fury.io/py/sonar.svg)](https://badge.fury.io/py/sonar)


# sonar

Tool to profile usage of HPC resources by regularly probing processes using
`ps`.

All it really does is to run `ps -e --no-header -o
pid,user:22,pcpu,pmem,size,comm` under the hood, and then filters and groups
the output and prints it to stdout, comma-separated.


## Changes since v0.5.0

You can find the old code on
https://github.com/NordicHPC/sonar/tree/with-slurm-data.  Since then, the code
has been simplified and the part that queried Slurm information has been
removed. The reason for the removal was that as we went to more and more nodes,
this could overload Slurm.

**This tool focuses on how resources are used**. What is actually running.  Its
focus is not (anymore) whether and how resources are under-used compared to
Slurm allocations. This is an important question but for another tool.


## Installation

Ideally install into a virtual environment:
```
$ pip install sonar
```

The tool does **not** need root permissions.

If you develop sonar, you can install like this:
```
$ git clone https://github.com/nordichpc/sonar.git
$ cd sonar
$ virtualenv venv
$ source venv/bin/activate
$ pip install -r requirements.txt
$ flit install --symlink
```


## Security and robustness

The tool does **not** need root permissions.

It does not modify anything and only writes to stdout.

The only external command called by `sonar ps` is `ps -e --no-header -o
pid,user:22,pcpu,pmem,size,comm` and the tool gives up and stops if the latter
subprocess does not return within 2 seconds to avoid a pile-up of processes.


## Collect processes with `sonar ps`

Available options:
```console
$ sonar ps --help

Usage: sonar ps [OPTIONS]

  Take a snapshot of the currently running processes that use more than
  `cpu_cutoff_percent` cpu and `mem_cutoff_percent` memory and print it comma-
  separated to stdout.

Options:
  --cpu-cutoff-percent FLOAT  CPU consumption percentage cutoff.  [default:
                              0.5]
  --mem-cutoff-percent FLOAT  Memory consumption percentage cutoff.  [default:
                              0.5]
  --help                      Show this message and exit.
```

You want to **run this every 10 or 20 minutes on every compute node**.

Here is an example:
```console
$ sonar ps

2022-10-09T14:29:05.824096+02:00,somehost,12,someuser,somecode,3.5,636
2022-10-09T14:29:05.824096+02:00,somehost,12,someuser,anothercode,0.9,159
2022-10-09T14:29:05.824096+02:00,somehost,12,someuser,slack,0.5,763
2022-10-09T14:29:05.824096+02:00,somehost,12,someuser,something,6.3,700
2022-10-09T14:29:05.824096+02:00,somehost,12,someuser,firefox,8.0,2577
2022-10-09T14:29:05.824096+02:00,somehost,12,someuser,alacritty,1.1,190
```

The columns are:
- time stamp
- hostname
- number of cores on this node
- user
- process
- CPU percentage (this is a 12-core node)
- memory used in MB


## Authors

- Henrik Rojas Nagel
- Mathias Bockwoldt
- [Radovan Bast](https://bast.fr)


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

**Do not interact with Slurm at all**:
The initial version correlated information we gathered from `ps` (what is
actually running) with information from Slurm (what was requested). This was
useful and nice to have but became complicated to maintain since Slurm could
become unresponsive and then processes were piling up. In later versions this
got removed.  Job efficiency based on Slurm data (e.g. `seff`) should be
collected with a separate tool.  "Do one thing only and do it well".

**Why not also recording the `pid`**?:
Because we sum over processes of the same name that may be running over many
cores to have less output so that we can keep logs in plain text
([csv](https://en.wikipedia.org/wiki/Comma-separated_values)) and don't have to
maintain a database or such.


## How we run sonar on a cluster

We let cron execute the following script every 5 minutes on every compute node:
```bash
#!/usr/bin/env bash

set -euf -o pipefail

sonar_directory=/cluster/shared/sonar/data

current_year=$(date +'%Y')

mkdir -p ${sonar_directory}/${current_year}

/cluster/bin/sonar ps >> ${sonar_directory}/${current_year}/${HOSTNAME}.csv
```


## Similar and related tools

- Reference implementation which serves as inspiration:
  <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)
- [Ganglia Monitoring System](http://ganglia.info/)
