.. image:: https://github.com/NordicHPC/sonar/workflows/Test/badge.svg
   :target: https://github.com/NordicHPC/sonar/actions
.. image:: https://img.shields.io/badge/license-%20GPL--v3.0-blue.svg
   :target: LICENSE
.. image:: https://badge.fury.io/py/sonar.svg
   :target: https://badge.fury.io/py/sonar


sonar
=====

Tool to profile usage of HPC resources by regularly probing processes
using ``ps``.

.. contents:: Table of contents


Overview
--------

The code can do two things: take snapshots (``sonar snap``, typically every 20
minutes or so), and map them (``sonar map``, whenever you like) to
applications/projects/users::

  $ sonar --help

  usage: sonar [-h]  ...

  Tool to profile usage of HPC resources by regularly probing processes using ps.

  optional arguments:
    -h, --help  show this help message and exit

  Subcommands:

      snap      Take a snapshot of the system. Run this on every node and often
                (e.g. every 20 minutes).
      map       Parse the system snapshots and map applications. Run this only
                once centrally and typically once a day.

  Run sonar <subcommand> -h to get more information about subcommands.


Similar and related tools
-------------------------

-  Reference implementation which serves as inspiration:
   https://github.com/UNINETTSigma2/appusage
-  `TACC Stats <https://github.com/TACC/tacc_stats>`__
-  `sonar-web <https://github.com/NordicHPC/sonar-web>`__: Plots
   daily/weekly/monthly CPU usage summary for clusters.


Authors
-------

-  Henrik Rojas Nagel
-  Mathias Bockwoldt
-  `Radovan Bast <https://bast.fr>`__


Installation
------------

Ideally install into a virtual environment::

  $ pip install sonar

If you develop sonar, you can install like this::

  $ git clone https://github.com/nordichpc/sonar.git
  $ cd sonar
  $ virtualenv venv
  $ source venv/bin/activate
  $ pip install -r requirements.txt
  $ flit install --symlink


Using sonar
-----------

-  `Taking snapshots with sonar snap <doc/usage/snap.md>`__
-  `Map processes to applications with sonar map <doc/usage/map.md>`__


Design goals and design decisions
---------------------------------

- Pip installable
- Minimal overhead for recording
- Super quick reporting and dashboard, both stdout and csv for web
  postprocessing
- Can be used as health check tool

``ps`` instead of ``top``:

We started using ``top`` but it turned out that ``top`` is dependent on
locale, so it displays floats with comma instead of decimal point in
many non-English locales. ``ps`` always uses decimal points. In
addition, ``ps`` is (arguably) more versatile/configurable and does not
print the header that ``top`` prints. All these properties make the
``ps`` output easier to parse than the ``top`` output.
