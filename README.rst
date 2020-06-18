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


-  Reference implementation which serves as inspiration:
   https://github.com/UNINETTSigma2/appusage
-  `Design goals and design decisions <doc/design.md>`__
-  Status: alpha, API might change, but we already use it on one cluster
-  `Contributing <doc/contributing.md>`__

Similar tools
-------------

-  `TACC Stats <https://github.com/TACC/tacc_stats>`__

Related tools which integrate with Sonar
----------------------------------------

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

-  `Overview <doc/usage/overview.md>`__
-  `Taking snapshots with sonar snap <doc/usage/snap.md>`__
-  `Map processes to applications with sonar map <doc/usage/map.md>`__
