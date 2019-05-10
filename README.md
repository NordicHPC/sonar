[![image](https://travis-ci.org/uit-no/sonar.svg?branch=master)](https://travis-ci.org/uit-no/sonar/builds)
[![image](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)


# sonar

Tool to profile usage of HPC resources by regularly probing processes
using `ps`.

- Reference implementation which serves as inspiration: <https://github.com/UNINETTSigma2/appusage>
- [Design goals and design decisions](doc/design.md)
- [Roadmap](doc/roadmap.md) (not yet production ready, but soon)
- [Contributing](doc/contributing.md)


## Authors

- Henrik Rojas Nagel
- Mathias Bockwoldt
- [Radovan Bast](https://bast.fr)


## Installation

Soon (TM) we will share the code via PyPI and then installation will
become simpler. Until then:

```
$ git clone https://github.com/uit-no/sonar.git
$ cd sonar
$ virtualenv venv
$ source venv/bin/activate
$ pip install -e .
```


## Using sonar

- [Overview](doc/usage.md#overview)
- [Taking snapshots with sonar snap](doc/usage.md#taking-snapshots-with-sonar-snap)
- [Running sonar snap on a cluster](doc/usage.md#running-sonar-snap-on-a-cluster)
- [Map processes to applications with sonar map](doc/usage.md#map-processes-to-applications-with-sonar-map)
