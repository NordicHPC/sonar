# Sonar root runner stub

sonar-daemon-runner is a simple program that is to be run as root.  It will fork, drop its
privileges down to the level of a named user, and then exec the Sonar daemon with the lowered
privoleges.  The sonar-daemon-runner remains resident and will receive requests from Sonar over a
pipe, and send back results.  The requests are for information that is only available to root.

For information about how to set it up, see the "Running as root" section of
[../doc/HOWTO-DAEMON.md](../doc/HOWTO-DAEMON.md).

(Currently the only implemented information request is for the /proc/PID/exe link, which names the
executable being run by the process, but there are other possibilities.)

The point of sonar-daemon-runner is that it is small and auditable (written in C with only libc, and
with very limited functionality); nobody has to entrust Sonar (and all its very many dependencies)
with root privileges.

To audit the code, it is necessary to look at sonar-daemon-runner.c and its dependencies, see the
Makefile.  Much of the code in this directory is test code that is not included in the production
build and need not be audited or trusted.

Generally, "make" will make sonar-daemon-runner for production; "make test" will build and run the
selftests; and then there is an interactive test, `req_exe_for_pids.bash`, that will test the
functionality end-to-end with a mock Sonar as client.
