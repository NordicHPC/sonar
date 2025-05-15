# Design, security, etc

## Early design goals and design decisions

- Easy installation
- Minimal overhead for recording
- Can be used as health check tool
- Does not need root permissions

**Use `ps` instead of `top`**:
We started using `top` but it turned out that `top` is dependent on locale, so
it displays floats with comma instead of decimal point in many non-English
locales. `ps` always uses decimal points. In addition, `ps` is (arguably) more
versatile/configurable and does not print the header that `top` prints. All
these properties make the `ps` output easier to parse than the `top` output.

**Do not interact with the Slurm database at all**:
The initial version correlated information we gathered from `ps` (what is
actually running) with information from Slurm (what was requested). This was
useful and nice to have but became complicated to maintain since Slurm could
become unresponsive and then processes were piling up.

**Why not also recording the `pid`**?:
Because we sum over processes of the same name that may be running over many
cores to have less output so that we can keep logs in plain text
([csv](https://en.wikipedia.org/wiki/Comma-separated_values)) and don't have to
maintain a database or such.


## Later design goals and design decisions

The needs of [Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and some bug fixes have led to
some feature creep (more data are reported), a bit of redesign (go directly to `/proc`, do not run
`ps`), and some quirky semantics (`cpu%` is only a good number for the first data point but is still
always reported, and `cputime/sec` is reported to complement it; and there's a distinction between
virtual and real memory that is possibly more useful on GPU-full and interactive systems than on HPC
CPU-only compute nodes).


## Security and robustness

The tool does **not** need root permissions.  It does not modify anything and writes output to
stdout (and errors to stderr).

No external commands are called by `sonar ps` or `sonar sysinfo`: Sonar reads `/proc` and probes the
GPUs via their manufacturers' SMI libraries to collect all data.

The Slurm `sacct` command is currently run by `sonar slurm` and `sinfo` is run by `sonar cluster`.
A timeout mechanism is in place to prevent these commands from hanging indefinitely.

Optionally, `sonar` will use a lockfile to avoid a pile-up of processes.


## Dependencies and updates

Sonar runs everywhere and all the time, and even though it currently runs without privileges it
strives to have as few dependencies as possible, so as not to become a target through a supply chain
attack.  There are some rules:

- It's OK to depend on libc and to incorporate new versions of libc
- It's better to depend on something from the rust-lang organization than on something else
- Every dependency needs to be justified
- Every dependency must have a compatible license
- Every dependency needs to be vetted as to active development, apparent quality, test cases
- Every dependency update - even for security issues - is to be considered a code change that needs review
- Remember that indirect dependencies are dependencies for us, too, and need to be treated the same way
- If in doubt: copy the parts we need, vet them thoroughly, and maintain them separately

There is a useful discussion of these matters [here](https://research.swtch.com/deps).

Unfortunately, these rules went out the window when we added Kafka support: this pulled in about 35
new dependencies, most of which are not vettable.  Users who don't need Kafka can remove it and
likely will see the number of dependencies drop to about three.
