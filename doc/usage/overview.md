

# Overview

The code can do two things: take snapshots (`sonar snap`, typically every 20
minutes or so), and map them (`sonar map`, whenever you like) to
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

Run sonar <subcommand> -h to get more information about subcommands.
```
