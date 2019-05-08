

# Using sonar

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
              (e.g. every 15 minutes).
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

2019-05-08T15:54:06.292155+0200,c61-8,someuser,someproject,1602448,oceanM,1598.1,1539
2019-05-08T15:54:06.292155+0200,c61-8,me,-,-,sonar,16.5,0
2019-05-08T15:54:06.292155+0200,c61-8,me,-,-,ps,1.0,0
```

The columns are: time stamp, hostname, user, Slurm project, Slurm job
id, process, CPU percentage (this is a multi-core node), and memory
percentage (again, multi-core node).

By default they are tab-separated but here I chose to display the result
comma-separated. You can also change cutoffs to not measure the tool
itself (`sonar snap --help`).

It can be useful to redirect the result to a file:

```
$ sonar snap >> /home/user/tmp/example.tsv
```


## Map processes to applications with sonar map

Map processes to applications:

```
$ sonar map --input-dir /home/user/tmp/
```
