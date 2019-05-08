

# Roadmap

## General considerations

- We start with saving everything into plain-text files. Later, we
  might use a database of some sort.
- The code should be agnostic of any machine-specifics, except that it
  expects Linux with standard tools. Even Slurm should be optional.
  - The idea is that it should be debuggable as far as possible on
    our local computers.
- All machine-specific stuff should be in config files in [YAML format](https://en.wikipedia.org/wiki/YAML).
- Config folder/files and output files should be possible to specify
  by command line arguments.
- Usage of Slurm should be possible to specify by command line
  argument.
- We should have a (rough) plan on how long we should keep files and
  how and when to clean old files.


### Python-specific

- Use Python 3 (Python 2 is dead (as is Python 1))
- Try to be compliant with PEP8 (including 4 spaces)
- Flat hierarchy (no classes or as few as possible)
- Create a pip-installable module
- Use tests (`pytest` and Travis CI)
- Use `pycodestyle` to enforce a somewhat consistent code style


## Module structure

- Sonar should consist of three parts:
  - Data gathering → This just gathers data and saves it. No
    "above-basic" parsing or processing. Should be fast and be able
    to run e.g. every 30 minutes.
  - Processing → This parses the files written by the gatherer and
    creates standardized output depending on the use-case. Speed is
    not too important, should run e.g. daily.
  - Visualising → This accepts the processed data and presents it
    depending on the use-case (website, stdout, or csv)


### Module 1: Data gathering

- Running regularly, e.g. hourly or every 30 minutes
- Run `ps` to gather running processes.
- These processes should be filtered by given users (`root`, `nobody`,
  `syslog`, ...)
- The shell command might be extended to also gather memory usage.
- An optional call to `squeue` could gather projects
- Using the `squeue` call, we may find stray processes (good old
  `gaussian`, for example)
- In the end, we should save (at least):
  - Date-time in ISO 8601 with time zone:
    2018-11-29T12:05:47.123456+01:00
  - hostname
  - username
  - optional project from Slurm (or `-` if no project or no Slurm)
  - command
  - memory and cpu usage info
- Preliminary format for saving is tab-separated values (tsv)
- We will append new entries to files and not create a new file for
  each snapshot.


### Module 2: Processing

- Running regularly, e.g. daily
- The commands from `ps` have to be mapped onto their respective
  common program name.
  - The mapping should be readable from csv/tsv.
  - The mapping should work with regular expressions.
  - We might want to think about performance, since processing huge
    amounts of data (e.g. hourly data of a whole year) with regular
    expressions may take ... very long. Random idea(s): Use a cache
    for the plain-string-to-program mapping since many programs run
    with exactly the same command string. This also catches programs
    running for weeks on several nodes.
  - Currently, `ps` is called such that it only gives the command
    without arguments. That means that scripts/programs called with
    e.g. `python my_fancy_app` are just recognized as `python`. The
    question is, if we need more detail. Also, the commands with
    arguments will possibly yield more false assignments, simply
    because the strings are longer. It is possible to save both, the
    command name and the command with arguments as two columns in
    the output of `ps`. For many programs, the command is sufficient
    and for scripts, etc., the arguments could be evaluated.
    Downside would be that the output gets *much* larger with all
    arguments. Another problem is that `ps` `comm` (without
    arguments) and `command` (with arguments) may both include
    spaces. This could be tricky to parse. Should be doable by
    parsing fixed-width columns instead of simple `split()`.
  - Commands will probably have to be mapped in order: from more
    specific to less specific.
- It may be desirable to allow for some hierarchy or tags for the
  programs.
  - This would allow users to group e.g. "chemical programs" or
    "licensed programs".
  - May be included here or in module 3.
- Stats for (configurable) time frames should be calculated (this
  allocation period, this month, last 30 days, ...).
  - The total usage (cpu) of every program in the given period(s)
    should be saved.
  - A distribution of needed memory per program at the snapshots
    might be interesting (total usage does not make sense for
    memory).
- Output should be json and csv/tsv.
  - json for programmatic access (web, shell), csv/tsv for manual
    access (tsv only on demand in module 3?).


### Module 3: Visualising

- Running only on demand
- Serving a web dashboard probably using `flask`.
