# Some design notes of historical interest only

## Intermediate (ca 2023/2024) design goals and design decisions

Relative to the "early" goals (below), the needs of
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) and some bug fixes led to some feature creep
(more data were reported), a bit of redesign (Sonar would go directly to `/proc`, do not run `ps`),
and some quirky semantics (`cpu%` is only a good number for the first data point but is still always
reported, and `cputime/sec` is reported to complement it; and there's a distinction between virtual
and real memory that is possibly more useful on GPU-full and interactive systems than on HPC
CPU-only compute nodes).

Other than that, the Intermediate goals were a mix of early goals and the current (2025/2026)
requirements, see [DESIGN.md](DESIGN.md).

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
