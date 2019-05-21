[![Builds](https://travis-ci.org/uit-no/sonar.svg?branch=master)](https://travis-ci.org/uit-no/sonar/builds)
[![License](https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg)](LICENSE)
[![PyPI version](https://badge.fury.io/py/sonar.svg)](https://badge.fury.io/py/sonar)


# sonar

Tool to profile usage of HPC resources by regularly probing processes
using `ps`.

- Reference implementation which serves as inspiration: <https://github.com/UNINETTSigma2/appusage>
- [Design goals and design decisions](doc/design.md)
- Status: not yet production ready, but soon
- [Contributing](doc/contributing.md)


## Authors

- Henrik Rojas Nagel
- Mathias Bockwoldt
- [Radovan Bast](https://bast.fr)


## Installation

### Installing from PyPI

Ideally install into a virtual environment or Pipenv:

```
$ pip install sonar
```


### Installing from sources

```
$ git clone https://github.com/uit-no/sonar.git
$ cd sonar
$ virtualenv venv
$ source venv/bin/activate
$ pip install -e .
```


## Using sonar

- [Overview](doc/usage/overview.md)
- [Taking snapshots with sonar snap](doc/usage/snap.md)
- [Map processes to applications with sonar map](doc/usage/map.md)
