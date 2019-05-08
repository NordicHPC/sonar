.. image:: https://travis-ci.org/uit-no/sonar.svg?branch=master
   :target: https://travis-ci.org/uit-no/sonar/builds
.. image:: https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg
   :target: LICENSE


=====
sonar
=====

Tool to profile usage of HPC resources by regularly probing processes using `ps`.

Reference implementation which serves as inspiration:
https://github.com/UNINETTSigma2/appusage


Authors
=======

- Henrik Rojas Nagel
- Mathias Bockwoldt
- `Radovan Bast <https://bast.fr>`_


Design goals
============

- pip installable
- minimal overhead for recording
- super quick reporting and dashboard, both stdout and web
- can be used as health check tool
- data filtering/mapping is asynchronous

For more details please see `the roadmap <doc/roadmap.rst>`_. See also
our `design decisions <doc/design-decisions.rst>`_.


Installation
============

Soon (TM) we will share the code via PyPI and then installation will become simpler. Until then::

  $ git clone https://github.com/uit-no/sonar.git
  $ cd sonar
  $ virtualenv venv
  $ source venv/bin/activate
  $ pip install -e .


Using sonar
===========

Currently you can do two things with the code (soon more), take snapshots, and map them to applications/projects/users::

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

Take a snapshot::

  $ sonar snap >> /home/user/tmp/example.tsv

Map processes to applications::

  $ sonar map --input-dir /home/user/tmp/


Contributing
============

How to test your changes
------------------------

Before contributing code changes, please run the test set::

  $ pytest -vv -s sonar
