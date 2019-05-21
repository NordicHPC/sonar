

# Taking snapshots with sonar snap

This is me running `sonar snap` on a compute node:

```
$ sonar snap --output-delimiter ","

2019-05-10T17:11:34.585859+0200,c10-4,16,me,sonar,31.0,0,-,-,-,-
2019-05-10T17:11:34.585859+0200,c10-4,16,somebody,vasp.5.3.5,1506.4,5151,someproject,1598301,64,2000M
```

The columns are:
- time stamp
- hostname
- number of cores on this node
- user
- process
- CPU percentage (this is a 20-core node)
- memory used in MB
- Slurm project
- Slurm job ID
- Number of CPUs requested by the job
- Minimum size of memory requested by the job

By default they are tab-separated but here I chose to display the result
comma-separated. You can also change cutoffs or ignore users to not measure the tool
itself (`sonar snap --help`).

It can be useful to redirect the result to a file:

```
$ sonar snap >> /home/user/tmp/example.tsv
```

This is how it looks when I run `sonar snap` on my laptop (without Slurm):

```
$ sonar snap --output-delimiter ","

2019-05-11T14:54:16.940502+0200,laptop,4,root,Xorg,0.7,47,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,gnome-shell,0.7,188,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,pulseaudio,0.6,7,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,chromium,16.9,3283,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,fish,0.5,23,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,vim,0.6,7,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,sonar,23.0,23,-,-,-,-
2019-05-11T14:54:16.940502+0200,laptop,4,me,gnome-terminal-,0.9,47,-,-,-,-
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
current_year=$(date +'%Y')
mkdir -p /global/work/sonar/snap-outputs/${current_year}
sonar snap --ignored-users root >> /global/work/sonar/snap-outputs/${current_year}/${HOSTNAME}.tsv
```

This produces ca. 10 MB data per day.
