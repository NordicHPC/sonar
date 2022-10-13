[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)
[![image](https://badge.fury.io/py/sonar.svg)](https://badge.fury.io/py/sonar)


# sonar

Tool to profile usage of HPC resources by regularly probing processes using
`ps`.



## Changes since v0.5.0

You can find the old code on
https://github.com/NordicHPC/sonar/tree/with-slurm-data.  Since then, the code
has been simplified and the part that queried Slurm information has been
removed. The reason for the removal was that as we went to more and more nodes,
this could overload Slurm.

**This tool focuses on how resources are used**. What is actually running.  Its
focus is not (anymore) whether and how resources are under-used compared to
Slurm allocations. This is an important question but for another tool.


## Similar and related tools

- Reference implementation which serves as inspiration:
  <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)


## Authors

- Henrik Rojas Nagel
- Mathias Bockwoldt
- [Radovan Bast](https://bast.fr)


## Design goals and design decisions

- Pip-installable
- Minimal overhead for recording
- Can be used as health check tool

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


## Installation

Ideally install into a virtual environment:
```
$ pip install sonar
```

If you develop sonar, you can install like this:
```
$ git clone https://github.com/nordichpc/sonar.git
$ cd sonar
$ virtualenv venv
$ source venv/bin/activate
$ pip install -r requirements.txt
$ flit install --symlink
```

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


## Outdated: Overview

The code can do two things: take snapshots (`sonar probe`, typically
every 20 minutes or so), and map them (`sonar map`, whenever you like)
to applications/projects/users:

    $ sonar --help

    usage: sonar [-h]  ...

    Tool to profile usage of HPC resources by regularly probing processes using ps.

    optional arguments:
      -h, --help  show this help message and exit

    Subcommands:

        probe     Take a snapshot of the system. Run this on every node and often
                  (e.g. every 20 minutes).
        map       Parse the system snapshots and map applications. Run this only
                  once centrally and typically once a day.

    Run sonar <subcommand> -h to get more information about subcommands.


## Outdated: How to analyze sonar logs

For this run `sonar map` which will go through the logs, and map
processes to applications:

    $ sonar map --input-dir /home/user/folder/with/logs

By default you will see data for the past 7 days. But you can change
this:

    $ sonar map --input-dir /home/user/folder/with/logs --num-days 300

Sonar uses the following mapping files:
<https://github.com/nordichpc/sonar/tree/main/sonar/mapping>

The mapping files ([string\_map.txt]{.title-ref} and
[regex\_map.txt]{.title-ref}) contain a space-separated (does not matter
how many spaces) mapping from process to application.

You can use your own mapping files instead:

    $ sonar map --input-dir /home/user/folder/with/logs \
                --str-map-file /home/user/my-own-mapping/string_map.txt \
                --re-map-file /home/user/my-own-mapping/regex_map.txt

You are welcome to use your own but encouraged to contribute mappings to
<https://github.com/nordichpc/sonar/tree/main/sonar/mapping>.

You can also export daily, weekly, and monthly CPU load percentages in
CSV format for further postprocessing, e.g. using
<https://github.com/NordicHPC/sonar-web>:

    $ sonar map --input-dir /home/user/folder/with/logs --export-csv daily
    $ sonar map --input-dir /home/user/folder/with/logs --export-csv weekly --num-days 200


## Outdated: Running sonar probe on a cluster

We let cron execute a script every 20 minutes:

    10,30,50 * * * * /global/work/sonar/sonar/cron-sonar.sh

The script `cron-sonar.sh` creates a list of active nodes and executes
`run-probe.sh` on all of these nodes:

    #!/bin/bash

    SONAR_ROOT="/global/work/sonar"

    # get list of all available nodes
    /usr/bin/sinfo -h -r -o '%n' > ${SONAR_ROOT}/tmp/list-of-nodes 2> ${SONAR_ROOT}/tmp/list-of-nodes.err

    # run sonar probe on all available nodes
    /usr/bin/pdsh -w \^${SONAR_ROOT}/tmp/list-of-nodes ${SONAR_ROOT}/sonar/run-probe.sh >> ${SONAR_ROOT}/tmp/pdsh.log 2>> ${SONAR_ROOT}/tmp/pdsh.err

In `run-probe.sh` we load the Python environment and wrap around
`sonar probe`:

    #!/usr/bin/env bash

    source /global/work/sonar/python/environment
    pyenv shell 3.6.7

    source /global/work/sonar/sonar/venv/bin/activate
    current_year=$(date +'%Y')
    mkdir -p /global/work/sonar/probe-outputs/${current_year}
    sonar probe --ignored-users root >> /global/work/sonar/probe-outputs/${current_year}/${HOSTNAME}.tsv

This produces ca. 10 MB data per day.
