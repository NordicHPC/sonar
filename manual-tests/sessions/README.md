# Session batchless jobs

What we want to test:

- in a tree of processes below the session leader, the running time of the children of the root is
  accumulated to the root as the children exit

- the running time of the root is not accumulated to the session leader

To do this, we create a distinguished process S that is a new session leader (it calls setsid(2) to
make itself into one).  It forks off a process R that is the new job root.  R in turn forks off a
number of workers that do a lot of work, one after the other.

While we do this, sonar is running in the background with sufficient frequency to see what's going on.

The sonar records should show that work done by the W is accumulated into the R, and that when the R
exits, the time attributed to R is not accumulated into S.


mmul is just a payload.  On my dev system it runs in about 11s.

