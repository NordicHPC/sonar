.. image:: https://travis-ci.org/uit-no/sonar.svg?branch=master
   :target: https://travis-ci.org/uit-no/sonar/builds
.. image:: https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg
   :target: LICENSE


sonar
=====

Tool to profile usage of HPC resources by regularly probing processes using `ps`.

Reference implementation which serves as inspiration:
https://github.com/UNINETTSigma2/appusage


Design goals
------------

- pip installable
- minimal overhead for recording
- super quick reporting and dashboard, both stdout and web
- can be used as health check tool
- data filtering/mapping is asynchronous

For more details please see `the roadmap <doc/roadmap.rst>`_. See also
our `design decisions <doc/design-decisions.rst>`_.


Development
-----------

::

  $ virtualenv venv
  $ source venv/bin/activate
  $ pip install -e .
  $ sonar --help
  $ sonar snap >> /home/user/tmp/example.tsv
  $ sonar map --input-dir /home/user/tmp/
