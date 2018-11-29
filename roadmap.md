# Roadmap to appusage2

## General considerations

- We start with saving everything into plain-text files. Later, we might use a database of some sort.
- The code should be agnostic of any machine-specifics, except that it expects Linux with standard tools. Even Slurm should be optional.
  + The idea is that it should be debuggable as far as possible on our local computers.
- All machine-specific stuff should be in config files of some sort (json? csv?).
- Config folder/files and output files should be possible to specify by command line arguments.
- Usage of Slurm should be possible to specify by command line argument.


### Python-specific

- Use Python 3 (Python 2 is dead (as is Python 1))
- Try to be compliant with PEP8 (including 4 spaces)
- Flat hierarchy (no classes or as few as possible)
- Create a pip-installable module
- Use tests (TravisCI etc.)
- Use argparse


## Module structure

- Appusage should consist of three parts:
  + Data gathering → This just gathers data and saves it. No "above-basic" parsing or processing. Should be fast and be able to run e.g. hourly.
  + Processing → This parses the files written by the gatherer and creates standardized output depending on the use-case. Speed is not too important, should run e.g. daily.
  + Visualising → This accepts the processed data and presents it depending on the use-case (website, shell, Excel-sheet?)


### Module 1: Data gathering

- Run `top` or `ps` to gather running processes.
  + in original appusage: `ps H -e -opid=,user:20=,pcpu=,comm=`
- These processes should be filtered by standard Linux users (`root`, `nobody`, `syslog`, ...) either in Python or as part of the shell command, if possible.
- The shell command might be extended to also gather memory usage.
- An optional call to `squeue` could gather projects
- Using the `squeue` call, we may find stray processes (good old `gaussian`, for example)
- In the end, we should save (at least):
  + Date-time in ISO 8601 with time zone: 2018-11-29T12:05:47+01:00
  + hostname
  + username
  + optional project from Slurm (or `-` if no project or no Slurm)
  + command
  + maybe memory and cpu usage info
- Preliminary format for saving is tab-separated values (tsv)


### Module 2: Processing

- Todo


### Module 3: Visualising

- Todo
