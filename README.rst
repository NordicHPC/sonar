.. image:: https://travis-ci.org/uit-no/sonar.svg?branch=master
   :target: https://travis-ci.org/uit-no/sonar/builds
.. image:: https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg
   :target: LICENSE


sonar
=====

Tool to profile usage of HPC resources by regularly probing processes using `ps`.

Reference implementation which serves as inspiration:
https://github.com/UNINETTSigma2/appusage


Authors
-------

- Henrik Rojas Nagel
- Mathias Bockwoldt
- `Radovan Bast <https://bast.fr>`_


Design goals
------------

- pip installable
- minimal overhead for recording
- super quick reporting and dashboard, both stdout and web
- can be used as health check tool
- data filtering/mapping is asynchronous

For more details please see `the roadmap <doc/roadmap.rst>`_. See also
our `design decisions <doc/design-decisions.rst>`_.


Installation
------------

Soon (TM) we will share the code via PyPI and then installation will become simpler. Until then::

  $ git clone https://github.com/uit-no/sonar.git
  $ cd sonar
  $ virtualenv venv
  $ source venv/bin/activate
  $ pip install -e .


Quickstart for developers
-------------------------

Get help text::

  $ sonar --help

Take a snapshot::

  $ sonar snap >> /home/user/tmp/example.tsv

Map processes to applications::

  $ sonar map --input-dir /home/user/tmp/


How to test your changes
------------------------

Before contributing code changes, please run the test set::

  $ pytest -vv -s sonar
