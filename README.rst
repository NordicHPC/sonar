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

More details are in the `roadmap.md <roadmap.md>`_.


Development
-----------

::

  $ virtualenv venv
  $ source venv/bin/activate
  $ pip install -e .
  $ sonar-snap --help
  $ sonar-snap --output-file /tmp/example_output.tsv


Design decisions
----------------

We had some design decisions that were controversially discussed. To allow our
future selves or other developers to not go through the same struggle again,
they are shortly summarized.


ps instead of top
~~~~~~~~~~~~~~~~~

We started using ``top`` but it turned out that ``top`` is dependent on locale,
so it displays floats with comma instead of decimal point in many non-English
locales. ``ps`` always uses decimal points. In addition, ``ps`` is (arguably)
more versatile/configurable and does not print the header that ``top`` prints.
All these properties make the ``ps`` output easier to parse than the ``top``
output.
