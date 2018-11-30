.. image:: https://travis-ci.org/uit-no/sonar.svg?branch=master
   :target: https://travis-ci.org/uit-no/sonar/builds
.. image:: https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg
   :target: LICENSE


sonar
=====

Tool to profile usage of HPC resources by regularly probing processes using
ps/top.

Reference implementation which we serves as inspiration:
https://github.com/UNINETTSigma2/appusage


Development
-----------

::

  $ virtualenv venv
  $ source venv/bin/activate
  $ pip install -e .
  $ sonar-snap --help
  $ sonar-snap --output-file /tmp/example_output.tsv
