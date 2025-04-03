# Sonar output data formats

This file describes the "old" data format, a mix of CSV and somewhat unstructured JSON.  Eventually
this will be superseded entirely by the "new" format, a structured JSON format described in
NEW-FORMAT.md.

The documentation below is probably not complete.

## Preliminaries

### History

The value has changed somewhat over time.  This document is organized as a preamble followed by what
is effectively a historical record, not (yet) as a manual of the current state of affairs.  It may
be necessary to read a lot of it to get the full picture.  This situation will change.

### Values

All values are generally JSON-compatible: numbers are never NaN or Infinity; strings are Unicode
(UTF-8) and do not contain control characters.  All integer quantities will fit in 52 bits (and can
be represented exactly in a IEEE 64-bit float) but not always in 32 bits.  All floating point values
can be represented precisely in a 32-bit IEEE float.

### "Free CSV format"

The CSV format, used for all CSV output except the very earliest Sonar versions, prefixes each field
value with the field name, may omit zero or blank fields, and may present fields in any order, but
otherwise follows CSV conventions.  We call this "free CSV format":

```
v=0.7.0,time=2023-08-10T11:09:41+02:00,host=somehost,cores=8,user=someone,job=0,cmd=slack,cpu%=3.9,cpukib=716924,gpus=none,gpu%=0,gpumem%=0,gpukib=0,cputime_sec=266
```

When the data are nested, the default representation of the nested data is also free CSV.  (This
leads to an exponential blowup in the number of quotes used as quotes are doubled on every level,
but this is usually OK since structures are not nested deeply.)  The value
`{"a":{"x":1,"y":"v,w"},"b":{"z":3}}` will therefore be rendered as `a="x=1,y=""v,w""",b=z=3`.

## `sonar ps` JSON output

JSON output for `sonar ps` was introduced in v0.13 and is not the default.  The field names and
semantics are as for the CSV version, described below.  This section describes the overall data
layout.

For each run of `sonar ps` there will be an outer envelope object containing these fields:

* `v` - the semver version number of the Sonar executable
* `time`
* `host`
* `load` - an array of per-CPU load data, but not base-45 compressed as in the CSV data
* `gpuinfo` - an array of per-GPU load data
* `samples` - an array of sample values.  These are objects that contain the sampe data as the CSV
  data described below, but without the fields that are in the envelope.

The per-GPU load data are one object per GPU, with these fields, same as for the CSV data:

* `fan%`
* `mode` - compute mode, omitted if "Default"
* `perf`
* `musekib`
* `cutil%`
* `mutil%`
* `tempc`
* `poww`
* `powlimw`
* `cez`
* `memz`

Thus while the CSV format lays out the GPUs as an array-per-attribute, the JSON format lays it out
as an array of gpus with individual attributes.

## `sonar ps` CSV output

### Version 0.13.0 `ps` CSV output format

`gpuinfo` (optional, default blank): This is an encoding of sampled per-gpu resource usage.  The
format is a nested array of attributes, where attribute values are separated by `|`, usually it will
look like this:

```
"gpuinfo=fan%=27|28|28,perf=P8|P8|P8,musekib=1014720|269696|269696,tempc=26|27|28,poww=4|1|19,powlimw=250|250|250,cez=300|300|300,memz=405|405|405"
```

Attributes may be omitted if all the values are zero; individual values may be omitted if the values
are zero but the separator will still be present (the above example is from a node with three
cards).

The fields are:

`fan%` (integer) - speed of primary fan as percentage of max speed, this may sometimes exceed 100.

`mode` (string) - system-specific compute mode, omitted if "Default"

`perf` (string) - system-specific performance mode

`musekib` (integer) - amount of memory in use in kilobytes

`cutil%` (integer) - compute element utilization in percent

`mutil%` (integer) - memory utilization in percent

`tempc` (integer) - value of primary temperature sensor in degrees Celsius

`poww` (integer) - current power usage in watts

`powlimw` (integer) - current power limit in watts

`cez` (integer) - current compute element clock speed in MHz

`memz` (integer) - current memory clock speed in MHz

### Version 0.12.0 `ps` CSV output format

Version 0.12.0 adds one field:

`load` (optional, default blank): This is an encoding of the per-cpu time usage in seconds on the
node since boot.  It is the same for all records and is therefore printed only with one of them per
sonar invocation.  The encoding is an array of N+1 u64 values for an N-cpu node.  The first value is
the "base" value, it is to be added to all the subsequent values.  The remaining are per-cpu values
in order from cpu0 through cpuN-1.  Each value is encoded little-endian base-45, with the initial
character of each value chosen from a different set than the subsequent characters.  The character
sets are:

```
INITIAL = "(){}[]<>+-abcdefghijklmnopqrstuvwxyz!@#$%^&*_"
SUBSEQUENT = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ~|';:.?/`"
```

The base-45 digits of the value `897` are (in little-endian order) 42 and 19, and the encoding of
this value is thus `&J`.  As the initial character is from a different character set, no explicit
separator is needed in the array - the initial digit acts as a separator.

### Version 0.11.0 `ps` CSV output format

Version 0.11.0 adds one field:

`ppid` (optional, default "0"): The parent process ID of the job, a positive integer.


### Version 0.10.0 `ps` CSV output format

The fields `cores` and `memtotalkib` were removed, as they were unused by all clients and are
supplied by `sonar sysinfo` for clients that need that information.


### Version 0.9.0 `ps` CSV output format

Version 0.9.0 documents that the `user` field *in previous versions* could have the value
`_noinfo_`.  This value is sometimes observed in the output from older versions (though no clients
were looking for it).

Version 0.9.0 extends the encoding of the `user` field: it can now (also) have the value
`_noinfo_<uid>` where `<uid>` is the user ID, if user information was unobtainable for any reason
but we have a UID.  Clients could be able to handle both this encoding and the older encoding.


### Version 0.8.0 `ps` CSV output format

Fields with default values (zero in most cases, or the empty set of GPUs) are not printed.

Version 0.8.0 adds two fields:

`memtotalkib` (optional, default "0"): The amount of physical RAM on this host, a nonnegative
integer, with 0 meaning "unknown".

`rssanonkib` (optional, default "0"): The current CPU data "RssAnon" (resident private) memory in KiB,
a nonnegative integer, with 0 meaning "no data available".

Version 0.8.0 also clarifies that the existing `cpukib` field reports virtual data+stack memory, not
resident memory nor virtual total memory.

### Version 0.7.0 `ps` CSV output format

Each field has the syntax `name=value` where the names are defined below.  Fields are separated by
commas, and each record is terminated by a newline.  The syntax of the file is therefore as for CSV
(including all rules for quoting).  However the semantics do not adhere to strict CSV: there may be
a variable number of fields ("columns"), and as the fields are named, they need not be presented in
any particular order.  Not all of the fields may be present - they have default values as noted
below.  Consumers should assume that new fields may appear, and should not treat records with
unknown field names as errors.  Broadly we would like to guarantee that fields never change meaning.

Integer fields will tend to be truncated toward zero, not rounded or rounded up.

The field names and their meaning are:

`v` (required): The record version number, a semantic version number on the format `n.m.o`.

`time` (required): The time stamp of the sample, an ISO time format string without fractional
seconds but with TZO.  Every record created from a single invocation of `sonar` has the same
timestamp (consumers may depend on this).

`host` (required): The fully qualified domain name of the host running the job, an alphanumeric
string.  There is only a single host.  If the job spans hosts, there will be multiple records for
the job, one per host; see `job` below.

`user` (required): The local Unix user name of user owning the job, an alphanumeric string.  This
can also be `_zombie_<pid>` for zombie processes, where `<pid>` is the process ID of the process but
the user ID could not be obtained, or `_noinfo_<uid>`, where `<uid>` is the user ID of the process
but the user name could not be obtained.

`cmd` (required): The executable name of the process/command without command line arguments, an
alphanumeric string.  This can be `_unknown_` for zombie jobs, or `_noinfo_` for non-zombies when
the command name can't be found.

`cores` (optional, default "0", removed in v0.10): The number of cores on this host, a nonnegative
integer, with 0 meaning "unknown".

`memtotalkib` (optional, default "0", removed in v0.10): The amount of physical RAM on this host, a
nonnegative integer, with 0 meaning "unknown".

`job` (optional, default "0"): The job ID, a positive integer. This field will be 0 if the job or
process does not have a meaningful ID.  There may be many records for the same job, one for each
process in the job (subject to filtering); these records can have different host names too.
Processes in the same job on the same host are merged if the `--rollup` command line option is used
and the processes have the same `cmd` value.

NOTE CAREFULLY that if the job ID is 0 then the process record is for a unique job with unknown job
ID.  Multiple records with the job ID 0 should never be merged into a single job by the consumer.

`pid` (optional, default "0"): The process ID of the job, a positive integer.  For a rolled-up job
(see `rolledup` below) this has value zero.  Otherwise, this record represents one process and so
the field holds the process ID.

`ppid` (optional, default "0"): The parent process ID of the job, a positive integer.

`cpu%` (optional, default "0"): The running average CPU percentage over the true lifetime of the
process (ie computed independently of the sonar log), a nonnegative floating-point number.  100.0
corresponds to "one full core's worth of computation".

`cpukib` (optional, default "0"): The current CPU data+stack virtual memory used in KiB, a
nonnegative integer.

`gpus` (optional, default "none"): The list of GPUs currently used by the job, a comma-separated
list of GPU device numbers, all of them nonnegative integers.  The value can instead be `none` when
the process uses no GPUs, or `unknown` when the process is known to use GPUs but their device
numbers can't be determined.

`gpu%` (optional, default "0"): The current GPU percentage utilization summed across all cards, a
nonnegative floating-point number.  100.0 corresponds to "one full card's worth of computation".

`gpukib` (optional, default "0"): The current GPU memory used in KiB, a nonnegative integer.  This
is summed across all cards.

The difference between `gpukib` and `gpumem%` (below) is that, on some cards some of the time, it is
possible to determine one of these but not the other, and vice versa.  For example, on the NVIDIA
cards we can read both quantities for running processes but only `gpukib` for some zombies.  On the
other hand, on our AMD cards there is no support for detecting the absolute amount of memory used,
nor the total amount of memory on the cards, only the percentage of gpu memory used.  Sometimes we
can convert one figure to another, but other times we cannot quite do that.  Rather than encoding
the logic for dealing with this in sonar, the task is currently offloaded to the front end.

`gpumem%` (optional, default "0"): The current GPU memory usage percentage, a nonnegative
floating-point number.  This is summed across all cards.  100.0 corresponds to "one full card's
worth of memory".

`cputime_sec` (optional, default "0"): Accumulated CPU time in seconds that a process has used over
its lifetime, a nonnegative integer.  The value includes time used by child processes that have
since terminated.

`rolledup` (optional, default "0"): The number of additional processes with the same `job` and `cmd`
that have been rolled into this one in response to the `--rollup` switch.  That is, if the value is
`1`, the record represents the sum of the data for two processes.  If a record represents part of a
rolled-up job then this field must be present.

### Version 0.6.0 `ps` CSV output format (and earlier)

The fields in version 0.6.0 are unnamed and the fields are always presented in the same order.  The
fields have (mostly) the same syntax and semantics as the 0.7.0 fields, with these notable differences:

* The time field has a fractional-second part and is always UTC (the TZO is always +00:00)
* The `gpus` field is a base-2 binary number representing a bit vector for the cards used; for the `unknown` value, it is a string of `1` of length 32.

The order of fields is:

`time`, `host`, `cores`, `user`, `job`, `cmd`, `cpu%`, `cpukib`, `gpus`, `gpu%`, `gpumem%`, `gpukib`

where the fields starting with `gpus` may be absent and should be taken to have the defaults
presented above.

Earlier versions of `sonar` would always roll up processes with the same `job` and `cmd`, so older
records may or may not represent multiple processes' worth of data.

## `sonar sysinfo` JSON output

### Version 0.13 `sysinfo` JSON format

v0.13 adds much more detailed GPU information to the sysinfo output:

- `gpu_info` - array of per-gpu information

Where each GPU has these fields:

- `bus_addr` - card address on the local system (may change on boot)
- `index` - card index of the card (may change on boot)
- `uuid` - card UUID, or at least something mostly-unique, not dependent on where the card is on a node or ideally even on which node it's on, or on software versions
- `manufacturer` - manufacturer's name
- `model` - card model
- `arch` - card architecture
- `driver` - driver version
- `firmware` - firmware version
- `mem_size_kib` - total memory on the card
- `power_limit_watt` - current power limit set
- `max_power_limit_watt` - max value of power limit
- `min_power_limit_watt` - min value of power limit
- `max_ce_clock_mhz` - max value of compute element clock
- `max_mem_clock_mhz` - max value of memory clock

### Version 0.9.0 `sysinfo` JSON format

The JSON structure has these fields:

- `timestamp` - string, an ISO-format timestamp for when the information was collected
- `hostname` - string, the FQDN of the host
- `description` - string, a summary of the system configuration with model numbers and so on
- `cpu_cores` - number, the total number of virtual cores (sockets x cores-per-socket x threads-per-core)
- `mem_gb` - number, the amount of installed memory in GiB (2^30 bytes)
- `gpu_cards` - number, the number of installed accelerator cards
- `gpumem_gb` - number, the total amount of installed accelerator memory across all cards in GiB

Numeric fields that are zero may or may not be omitted by the producer.

Note the v0.9.0 `sysinfo` output does not carry a version number.

## `sonar sysinfo` CSV output

The CSV format was introduced in v0.13 as a consquence of generalizing the output layer.  The CSV
rendering is a literal rendering of the JSON format, using the standard nesting syntax described
earlier.

## `sonar slurm` JSON output

JSON output for `sonar slurm` was introduced in v0.13 and is not the default.  The field names and
semantics are as for the CSV version, described below.

*Successful* slurm data are described by an envelope:

```
{"v":<version>,
 "jobs":[<job>, ...]}
```

where `<version>` is the semver of the sonar executable and each `<job>` is described by a flat
record with all fields that are present in the CSV output (below), except the `"v"` field.  All
fields are represented as quoted string values, even when logically numeric.


*Unsuccessful* slurm data are described by an object with `v`, `error`, and `timestamp` fields.

## `sonar slurm` CSV output

Each successful record contains these fields, where the fields other than `v` have the exact meaning
that `sacct` gives them, except:

* Fields with zero values are omitted (except for `JobName`, `Account`, and `User`).  Zero values are
  `Unknown`, `0`, `00:00:00`, `0:0`, and `0.00M`
* Timestamps are reformatted as ISO 8601 with a proper TZO (Slurm timestamps don't carry time zone
  information).

* `v`: semver of the Sonar executable
* `JobID`
* `JobIDRaw`
* `User`
* `Account`
* `State`
* `Start`
* `End`
* `AveCPU`
* `AveDiskRead`
* `AveDiskWrite`
* `AveRSS`
* `AveVMSize`
* `ElapsedRaw`
* `ExitCode`
* `Layout`
* `MaxRSS`
* `MaxVMSize`
* `MinCPU`
* `ReqCPUS`
* `ReqMem`
* `ReqNodes`
* `Reservation`
* `Submit`
* `Suspended`
* `SystemCPU`
* `TimelimitRaw`
* `UserCPU`
* `NodeList`
* `Partition`
* `AllocTRES`
* `Priority`
* `JobName`

If the run was unsuccessful then there are three fields:

* `v`: semver of the Sonar executable
* `error`: an error string
* `timestamp`: the timestamp of the run that failed

