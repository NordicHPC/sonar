[![image](https://github.com/NordicHPC/sonar/workflows/Test/badge.svg)](https://github.com/NordicHPC/sonar/actions)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)

# Sonar

Sonar is a tool to profile usage of HPC resources by regularly sampling processes, jobs, accelerators,
nodes, queues, and clusters.

Sonar examines `/proc` and `/sys` and/or runs diagnostic programs, filters and groups the
information, and prints it to stdout, stores it in a local directory tree, or sends it to a remote
collector.

Sonar proper is GPL-3 but some side components that are crucial for the interaction with other tools
that might not be GPL carry the MIT license.

![image of a fish swarm](img/sonar-small.png)

Image: [Midjourney](https://midjourney.com/), [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/legalcode)

## Documentation

Start by reading [the user manual](doc/MANUAL.md), which explains most things about what it can do
and how you make it do it.

For a deeper dive into how it works, try [the design document](doc/DESIGN.md).

To build it, or to modify it, try [the developer document](doc/HOWTO-DEVELOP.md).

A sample deployment of Sonar on a cluster and a data aggregator on a backend is outlined in
[doc/HOWTO-DEPLOY.md](doc/HOWTO-DEPLOY.md).

## Collecting and analyzing the data

Sonar's output data are rigorously specified and you can build your own data collectors,
post-processors and analyses, but you can also use these existing tools (both under active development):

* [JobAnalyzer](https://github.com/NAICNO/Jobanalyzer) allows Sonar logs to be queried and analyzed, and
  provides dashboards, interactive and batch queries, and reporting of system activity, policy violations,
  hung jobs, and more.
* [Slurm-monitor](https://github.com/2maz/slurm-monitor) is complementary to JobAnalyzer and focuses
  on managing and analyzing slurm queues and clusters, and has a benchmarking facility and other
  tools for job placement.

## Authors

- [Radovan Bast](https://bast.fr)
- Mathias Bockwoldt
- [Lars T. Hansen](https://github.com/lars-t-hansen)
- Henrik Rojas Nagel
- [Thomas Roehr](https://github.com/2maz)

## Similar and related tools

Sonar's original vision was to be a very simple, lightweight tool that did some basic things fairly
cheaply and produced easy-to-process output for subsequent scripting.  Sonar is no longer that: with
GPU integration, SLURM integration, Kafka exfiltration, memory-resident modes, structured output,
continual focus on performance and several elaborate backends, it is becoming as complex as the
tools it was intended to replace or compete with.

Here are some of those tools:

- [Trailblazing Turtle](https://github.com/guilbaults/TrailblazingTurtle), SLURM-specific but similar to Sonar.
- [Scaphandre](https://hubblo-org.github.io/scaphandre-documentation/index.html), for energy monitoring.
- [Sysstat and SAR](https://github.com/sysstat/sysstat), for monitoring a lot of things.
- [seff](https://support.schedmd.com/show_bug.cgi?id=1611), SLURM-specific.
- [TACC Remora](https://github.com/tacc/remora)
- Reference implementation which serves as inspiration: <https://github.com/UNINETTSigma2/appusage>
- [TACC Stats](https://github.com/TACC/tacc_stats)
- [Ganglia Monitoring System](http://ganglia.info/)
