

# Using sonar

## Overview

Currently you can do two things with the code (soon more), take
snapshots (`sonar snap`), and map them (`sonar map`) to
applications/projects/users:

```
$ sonar --help

usage: sonar [-h]  ...

Tool to profile usage of HPC resources by regularly probing processes using ps.

optional arguments:
  -h, --help  show this help message and exit

Subcommands:

    snap      Take a snapshot of the system. Run this on every node and often
              (e.g. every 20 minutes).
    map       Parse the system snapshots and map applications. Run this only
              once centrally and typically once a day.
    web       Run the web frontend to visualize results. This can run locally
              or on a server (via uWSGI).

Run sonar <subcommand> -h to get more information about subcommands.
```


## Taking snapshots with sonar snap

This is me running `sonar snap` on a compute node:

```
$ sonar snap --output-delimiter ","

2019-05-08T15:54:06.292155+0200,c61-8,20,someuser,someproject,1602448,oceanM,1598.1,1539
2019-05-08T15:54:06.292155+0200,c61-8,20,me,-,-,sonar,16.5,0
2019-05-08T15:54:06.292155+0200,c61-8,20,me,-,-,ps,1.0,0
```

The columns are: time stamp, hostname, number of cores on this node, user, Slurm project, Slurm job
id, process, CPU percentage (this is a 20-core node), and memory
percentage (again, 20-core node).

By default they are tab-separated but here I chose to display the result
comma-separated. You can also change cutoffs to not measure the tool
itself (`sonar snap --help`).

It can be useful to redirect the result to a file:

```
$ sonar snap >> /home/user/tmp/example.tsv
```


## Running sonar snap on a cluster

We let cron execute a script every 20 minutes:

```
10,30,50 * * * * /global/work/sonar/sonar/cron-sonar.sh
```

The script `cron-sonar.sh` creates a list of active nodes and executes `run-snap.sh` on all of these nodes:

```bash
#!/bin/bash

SONAR_ROOT="/global/work/sonar"

# get list of all available nodes
/usr/bin/sinfo -h -r -o '%n' > ${SONAR_ROOT}/tmp/list-of-nodes 2> ${SONAR_ROOT}/tmp/list-of-nodes.err

# run sonar snap on all available nodes
/usr/bin/pdsh -w \^${SONAR_ROOT}/tmp/list-of-nodes ${SONAR_ROOT}/sonar/run-snap.sh >> ${SONAR_ROOT}/tmp/pdsh.log 2>> ${SONAR_ROOT}/tmp/pdsh.err
```

In `run-snap.sh` we load the Python environment and wrap around `sonar snap`:

```bash
#!/usr/bin/env bash

source /global/work/sonar/python/environment
pyenv shell 3.6.7

source /global/work/sonar/sonar/venv/bin/activate
sonar snap --ignored-users root >> /global/work/sonar/snap-outputs/${HOSTNAME}.tsv
```

This produces ca. 10 MB data per day.


## Map processes to applications with sonar map

Map processes to applications:

```
$ sonar map --input-dir /home/user/snap-outputs --str-map-file example-mapping/string_map.txt --re-map-file example-mapping/regex_map.txt
```

Mapping files (string_map.txt and regex_map.txt) contain a space-separated
(does not matter how many spaces) mapping from process to application.
Example mapping files: https://github.com/uit-no/sonar/tree/master/example-mapping

You are welcome to use your own but encouraged to contribute mappings to our example files.
