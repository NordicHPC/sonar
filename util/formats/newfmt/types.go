// This is a machine-processable and partly executable specification for the new JSON format for
// Sonar output.  It is processable into Rust code and Markdown docs by code in ../../process-doc.
//
// Use standard Go doc comments on types, fields, and constants.  The doc comments on the dummy
// types `_preamble` and `_postamble` are handled specially.

package newfmt

import (
	"errors"
)

// ## Introduction
//
// Five types of data are collected:
//
// * job and process sample data
// * node sample data
// * job data
// * node configuration data
// * cluster data
//
// The job and process sample data are collected frequently on every node and comprise information
// about each running job and the resource use of the processes in the job, for all pertinent
// resources (cpu, ram, gpu, gpu ram, power consumption, i/o).
//
// The node sample data are also collected on every node, normally at the same time as the job and
// process sample data, and comprise information about the overall use of resources on the node
// independently of jobs and processes, to the extent these can't be derived from the job and
// process sample data.
//
// The job data are collected on a master node and comprise information about the job that are not
// directly related to moment-to-moment resource usage: start and end times, time in the queue,
// allocation requests, completion status, billable resource use and so on.  (Only applies to
// systems with a job manager / job queue.)
//
// The node configuration data are collected occasionally on every node and comprise information
// about the current configuration of the node.
//
// The cluster data are collected occasionally on a master node and comprise information about the
// cluster that are related to how nodes are grouped into partitions and short-term node status; it
// complements node configuration data.
//
// NOTE: Nodes may be added to clusters simply by submitting data for them.
//
// NOTE: We do not yet collect some interesting cluster configuration data – how nodes, racks,
// islands are connected and by what type of interconnect; the type of attached I/O.  Clusters are
// added to the database through other APIs.
//
// ## Slurm
//
// The motivation for extracting Slurm data is to obtain data about a job that are not apparent
// from process samples, notably data about requested resources and wait time, and to store them in
// such a way that they can be correlated with samples for queries and for as long as we need them.
// Data already obtained by process sampling are not to be gotten from Slurm, including most
// performance data and information about resources that are observably used by the job.
//
// The working hypothesis is that:
//
//   - all necessary Slurm data can be extracted from a single node in the cluster, as they only
//     include data that Slurm collects and stores for itself
//   - the Slurm data collection can be performed by sampling, ie, by running data collection
//     periodically using externally available Slurm tools, and not by getting callbacks from Slurm
//
// That does not remove performance constraints, as the Slurm system is already busy and does not
// need an extra load from a polling client that runs often.  It does ease performance constraints,
// though, as access to the Slurm data store will not impact compute nodes and places a load only
// on administrative systems.  Even so, we may not assume that sampling can run very often, as the
// data volumes can be quite large.  (`squeue | wc` on fox just now yields 100KB of formatted data.)
//
// In principle, Sonar shall send data about a job at least three times: when the job is created
// and enters the PENDING state, when it enters the RUNNING state, and when it has completed (in
// any of a number of different states).  At each of those steps, it shall send data that are
// available at that time that have not been sent before; this includes data that may have changed
// (for example, the Priority may be sent with a PENDING record but if the priority changes later,
// it should be sent again the next time a data sample is sent).  In practice, there are two main
// complications: Sonar runs as a sampler and may not observe a job in all of those states, and it
// may run in a stateless mode in which it will be unable to know whether it has already sent some
// information about a job and can avoid sending it again.  (There is a discussion to be had about
// other events: priority changes, job suspension, job resize.)
//
// Therefore, the Slurm data that are transmitted must be assumed by the consumer to be both
// partial and potentially redundant.  Data in records with later timestamps generally override
// data from earlier records.
//
// ## Data format overall notes
//
// The output is a tree structure that is constrained enough to be serialized as
// [JSON](https://www.rfc-editor.org/rfc/rfc8259) and other likely serialization formats (protobuf,
// bson, cbor, a custom format, whatever).  It shall follow the [json:api
// specification](https://jsonapi.org/format/#document-structure).  It generally does not
// incorporate many size optimizations.
//
// It's not a goal to have completely normalized data; redundancies are desirable in some cases to
// make data self-describing.
//
// In a serialization format that allows fields to be omitted, all fields except union fields will
// have default values, which are zero, empty string, false, the empty object, or the empty array.
// A union field must have exactly one member present.
//
// Field values are constrained by data types described below, and sometimes by additional
// constraints described in prose.  Primitive types are as they are in Go: 64-bit integers and
// floating point, and Unicode strings.  Numeric values outside the given ranges, non-Unicode
// string encodings, malformed timestamps, malformed node-ranges or type-incorrect data in any
// field can cause the entire top-level object containing them to be rejected by the back-end.
//
// The word "current" in the semantics of a field denotes an instantaneous reading or a
// short-interval statistical measure; contrast "cumulative", which is since start of process/job
// or since system boot or some other fixed time.
//
// Field names generally do not carry unit information.  The units are included in the field
// descriptions, but if they are not then they should be kilobytes for memory, megahertz for
// clocks, watts for power, and percentage points for relative utilization measures.  (Here,
// Kilobyte (KB) = 2^10, Megabyte (MB) = 2^20, Gigabyte (GB) = 2^30 bytes, SI notwithstanding.)
//
// The top-level object for each output data type is the "...Envelope" object.
//
// Within each envelope, `Data` and `Errors` are exclusive of each other.
//
// Within each data object, no json:api `id` field is needed since the monitoring component is a
// client in spec terms.
//
// The errors field in an envelope is populated only for hard errors that prevent output from being
// produced at all. Soft/recoverable errors are represented in the primary data objects.
//
// MetadataObject and ErrorObject are shared between the various data types, everything else is
// specific to the data type and has a name that clearly indicates that.
//
// Some fields present in the older Sonar data are no longer here, having been deemed redundant or
// obsolete.  Some are here in a different form.  Therefore, while old and new data are broadly
// compatible, there may be some minor problems translating between them.
//
// If a device does not expose a UUID, one will be constructed for it by the monitoring component.
// This UUID will never be confusable with another device but it may change, eg at reboot, creating
// a larger population of devices than there is in actuality.
//
// ## Data format versions
//
// This document describes data format version "0".  Adding fields or removing fields where default
// values indicate missing values in the data format do not change the version number: the version
// number only needs change if semantics of existing fields change in some incompatible way.  We
// intend that the version will "never" change.
type _preamble int

// String value where an empty value is an error, not simply absence of data
type NonemptyString string

// Uint64 value where zero is an error, not simply absence of datga
type NonzeroUint uint64

// RFC3339 localtime+TZO with no sub-second precision: yyyy-mm-ddThh:mm:ss+hh:mm, "Z" for +00:00.
type Timestamp NonemptyString

// Timestamp, or empty string for missing data
type OptionalTimestamp string

// Dotted host name or prefix of same, with standard restrictions on character set.
type Hostname NonemptyString

// An unsigned value that carries two additional values: unset and infinite.  It has a more limited
// value range than regular unsigned.  The representation is: 0 for unset, 1 for infinite, and v+2
// for all other values v.
type ExtendedUint uint64

const (
	// The unset value
	ExtendedUintUnset ExtendedUint = 0

	// The infinite value
	ExtendedUintInfinite ExtendedUint = 1

	// The base value for finite, set values
	ExtendedUintBase ExtendedUint = 2
)

func (e ExtendedUint) ToUint() (uint64, error) {
	if e >= ExtendedUintBase {
		return uint64(e - ExtendedUintBase), nil
	}
	return 0, errors.New("Not a finite numeric value")
}

const (
	// The bias that we subtract from a timestamp to represent the epoch (it saves a few bytes per data
	// item).  This is technically not part of the spec, but it's hard for it not to be, because it is
	// exposed indirectly in the emitted data and it can't subsequently be moved forward, only backward.
	// The value represents 2020-01-01T00:00:00Z, somewhat arbitrarily.
	EpochTimeBase uint64 = 1577836800
)

// String-valued enum tag for the record type
type DataType NonemptyString

const (
	// The tag for "sample" records
	DataTagSample DataType = "sample"

	// The tag for "sysinfo" records
	DataTagSysinfo DataType = "sysinfo"

	// The tag for "job" records
	DataTagJobs DataType = "job"

	// The tag for "cluster" records
	DataTagCluster DataType = "cluster"
)

// Information about the data producer and the data format.  After the data have been ingested, the
// metadata can be thrown away and will not affect the data contents.
//
// NOTE: The `attrs` field can be used to transmit information about how the data were collected.
// For example, sometimes Sonar is run with switches that exclude system jobs and short-running
// jobs, and the data could record this.  For some agents, it may be desirable to report on eg
// Python version (slurm-monitor does this).
type MetadataObject struct {
	// The name of the component that generated the data (eg "sonar", "slurm-monitor")
	Producer NonemptyString `json:"producer"`

	// The semver of the producer
	Version NonemptyString `json:"version"`

	// The data format version
	Format uint64 `json:"format,omitempty"`

	// An array of generator-dependent attribute values
	Attrs []KVPair `json:"attrs,omitempty"`

	// EXPERIMENTAL / UNDERSPECIFIED.  An API token to be used with
	// Envelope.Data.Attributes.Cluster, it proves that the producer of the datum was authorized to
	// produce data for that cluster name.
	Token string `json:"token,omitempty"`
}

// Information about a continuable or non-continuable error.
type ErrorObject struct {
	// Time when the error was generated
	Time Timestamp `json:"time"`

	// A sensible English-language error message describing the error
	Detail NonemptyString `json:"detail"`

	// Canonical cluster name for node generating the error
	Cluster Hostname `json:"cluster"`

	// name of node generating the error
	Node Hostname `json:"node"`
}

// Carrier of arbitrary attribute data
type KVPair struct {
	// A unique key within the array for the attribute
	Key NonemptyString `json:"key"`

	// Some attribute value
	Value string `json:"value,omitempty"`
}

// The Sysinfo object carries hardware information about a node.
//
// NOTE: "Nodeinfo" would have been a better name but by now "Sysinfo" is baked into everything.
//
// NOTE: Also see notes about envelope objects in the preamble.
//
// NOTE: These are extracted from the node periodically, currently with Sonar we extract
// information every 24h and on node boot.
//
// NOTE: In the Go code, the JSON representation can be read with ConsumeJSONSysinfo().
type SysinfoEnvelope struct {
	// Information about the producer and data format
	Meta MetadataObject `json:"meta"`

	// Node data, for successful probes
	Data *SysinfoData `json:"data,omitempty"`

	// Error information, for unsuccessful probes
	Errors []ErrorObject `json:"errors,omitempty"`
}

// System data, for successful sysinfo probes
type SysinfoData struct {
	// Data tag: The value "sysinfo"
	Type DataType `json:"type"`

	// The node data themselves
	Attributes SysinfoAttributes `json:"attributes"`
}

// This object describes a node, its CPUS, devices, topology and software
//
// For the time being, we assume all cores on a node are the same. This is complicated by eg
// BIG.little systems (performance cores vs efficiency cores), for one thing, but that's OK.
//
// NOTE: The node may or may not be under control of Slurm or some other batch system.
// However, that information is not recorded with the node information, but with the node sample
// data, as the node can be added to or removed from a Slurm partition at any time.
//
// NOTE: The number of physical cores is sockets * cores_per_socket.
//
// NOTE: The number of logical cores is sockets * cores_per_socket * threads_per_core.
type SysinfoAttributes struct {
	// Time the current data were obtained
	Time Timestamp `json:"time"`

	// The canonical cluster name
	Cluster Hostname `json:"cluster"`

	// The name of the host as it is known to itself
	Node Hostname `json:"node"`

	// Operating system name (the `sysname` field of `struct utsname`)
	OsName NonemptyString `json:"os_name"`

	// Operating system version (the `release` field of `struct utsname`)
	OsRelease NonemptyString `json:"os_release"`

	// Architecture name (the `machine` field of `struct utsname`)
	Architecture NonemptyString `json:"architecture"`

	// Number of CPU sockets
	Sockets NonzeroUint `json:"sockets"`

	// Number of physical cores per socket
	CoresPerSocket NonzeroUint `json:"cores_per_socket"`

	// Number of hyperthreads per physical core
	ThreadsPerCore NonzeroUint `json:"threads_per_core"`

	// Manufacturer's model name
	CpuModel string `json:"cpu_model"`

	// Primary memory in kilobytes
	Memory NonzeroUint `json:"memory"`

	// Base64-encoded SVG output of `lstopo`
	TopoSVG string `json:"topo_svg,omitempty"`

	// Per-card information
	Cards []SysinfoGpuCard `json:"cards,omitempty"`

	// Per-software-package information
	Software []SysinfoSoftwareVersion `json:"software,omitempty"`
}

// Per-card information.
//
// NOTE: Many of the string values are idiosyncratic or have card-specific formats, and some are
// not available on all cards.
//
// NOTE: Only the UUID, manufacturer, model and architecture are required to be stable over time
// (in practice, memory might be stable too).
//
// NOTE: Though the power limit can change, it is reported here (as well as in sample data) because
// it usually does not.
type SysinfoGpuCard struct {
	// Local card index, may change at boot
	Index uint64 `json:"index"`

	// UUID as reported by card.  See notes in preamble
	UUID string `json:"uuid"`

	// Indicates an intra-system card address, eg PCI address
	Address string `json:"address,omitempty"`

	// A keyword, "NVIDIA", "AMD", "Intel" (others TBD)
	Manufacturer string `json:"manufacturer,omitempty"`

	// Card-dependent, this is the manufacturer's model string
	Model string `json:"model,omitempty"`

	// Card-dependent, for NVIDIA this is "Turing", "Volta" etc
	Architecture string `json:"architecture,omitempty"`

	// Card-dependent, the manufacturer's driver string
	Driver string `json:"driver,omitempty"`

	// Card-dependent, the manufacturer's firmware string
	Firmware string `json:"firmware,omitempty"`

	// GPU memory in kilobytes
	Memory uint64 `json:"memory,omitempty"`

	// Power limit in watts
	PowerLimit uint64 `json:"power_limit,omitempty"`

	// Max power limit in watts
	MaxPowerLimit uint64 `json:"max_power_limit,omitempty"`

	// Min power limit in watts
	MinPowerLimit uint64 `json:"min_power_limit,omitempty"`

	// Max clock of compute element
	MaxCEClock uint64 `json:"max_ce_clock,omitempty"`

	// Max clock of GPU memory
	MaxMemoryClock uint64 `json:"max_memory_clock,omitempty"`
}

// The software versions are obtained by system-dependent means. As the monitoring component runs
// outside the monitored processes' contexts and is not aware of software that has been loaded with
// eg module load, the software reported in the software fields is thus software that is either
// always loaded and always available to all programs, or which can be loaded by any program but
// may or may not be.
//
// NOTE: For GPU software: On NVIDIA systems, one can look in $CUDA_ROOT/version.json, where the
// key/name/version values are encoded directly.  On AMD systems, one can look in
// $ROCm_ROOT/.info/.version*, where the file name encodes the component key and the file stores
// the version number. Clearly other types of software could also be reported for the node (R,
// Jupyter, etc), based on information from modules, say.
type SysinfoSoftwareVersion struct {
	// A unique identifier for the software package
	Key NonemptyString `json:"key"`

	// Human-readable name of the software package
	Name string `json:"name,omitempty"`

	// The package's version number, in some package-specific format
	Version NonemptyString `json:"version"`
}

// The "sample" record is sent from each node at each sampling interval in the form of a top-level
// sample object.
//
// NOTE: Also see notes about envelope objects in the preamble.
//
// NOTE: JSON representation can be read with ConsumeJSONSamples().
type SampleEnvelope struct {
	// Information about the producer and data format
	Meta MetadataObject `json:"meta"`

	// Sample data, for successful probes
	Data *SampleData `json:"data,omitempty"`

	// Error information, for unsuccessful probes
	Errors []ErrorObject `json:"errors,omitempty"`
}

// Sample data, for successful sysinfo probes
type SampleData struct {
	// Data tag: The value "sample"
	Type DataType `json:"type"`

	// The sample data themselves
	Attributes SampleAttributes `json:"attributes"`
}

// Holds the state of the node and the state of its running processes at a point in time, possibly
// filtered.
//
// NOTE: A SampleAttributes object with an empty jobs array represents a heartbeat from an idle
// node, or a recoverable error situation if errors is not empty.
type SampleAttributes struct {
	// Time the current data were obtained
	Time Timestamp `json:"time"`

	// The canonical cluster name whence the datum originated
	Cluster Hostname `json:"cluster"`

	// The name of the node as it is known to the node itself
	Node Hostname `json:"node"`

	// State of the node as a whole
	System SampleSystem `json:"system"`

	// State of jobs on the nodes
	Jobs []SampleJob `json:"jobs,omitempty"`

	// Recoverable errors, if any
	Errors []ErrorObject `json:"errors,omitempty"`
}

// This object describes the state of the node independently of the jobs running on it.
//
// NOTE: Other node-wide fields will be added (e.g. for other load averages, additional memory
// measures, for I/O and for energy).
//
// NOTE: The sysinfo for the node provides the total memory; available memory = total - used.
type SampleSystem struct {
	// The state of individual cores
	Cpus []SampleCpu `json:"cpus,omitempty"`

	// The state of individual GPU devices
	Gpus []SampleGpu `json:"gpus,omitempty"`

	// The amount of primary memory in use in kilobytes
	UsedMemory uint64 `json:"used_memory,omitempty"`
}

// The number of CPU seconds used by the core since boot.
type SampleCpu uint64

// This object exposes utilization figures for the card.
//
// NOTE: In all monitoring data, cards are identified both by current index and by immutable UUID,
// this is redundant but hopefully useful.
//
// NOTE: A card index may be local to a job, as Slurm jobs partition the system and may remap cards
// to a local name space.  UUID is usually safer.
//
// NOTE: Some fields are available on some cards and not on others.
//
// NOTE: If there are multiple fans and we start caring about that then we can add a new field, eg
// "fans", that holds an array of fan speed readings. Similarly, if there are multiple temperature
// sensors and we care about that we can introduce a new field to hold an array of readings.
type SampleGpu struct {
	// Local card index, may change at boot
	Index uint64 `json:"index"`

	// Card UUID.  See preamble for notes about UUIDs.
	UUID NonemptyString `json:"uuid"`

	// If not zero, an error code indicating a card failure state. code=1 is "generic failure".
	// Other codes TBD.
	Failing uint64 `json:"failing,omitempty"`

	// Percent of primary fan's max speed, may exceed 100% on some cards in some cases
	Fan uint64 `json:"fan,omitempty"`

	// Current compute mode, completely card-specific if known at all
	ComputeMode string `json:"compute_mode,omitempty"`

	// Current performance level, card-specific >= 0, or unset for "unknown".
	PerformanceState ExtendedUint `json:"performance_state,omitempty"`

	// Memory use in Kilobytes
	Memory uint64 `json:"memory,omitempty"`

	// Percent of computing element capability used
	CEUtil uint64 `json:"ce_util,omitempty"`

	// Percent of memory used
	MemoryUtil uint64 `json:"memory_util,omitempty"`

	// Degrees C card temperature at primary sensor (note can be negative)
	Temperature int64 `json:"temperature,omitempty"`

	// Watts current power usage
	Power uint64 `json:"power,omitempty"`

	// Watts current power limit
	PowerLimit uint64 `json:"power_limit,omitempty"`

	// Compute element current clock
	CEClock uint64 `json:"ce_clock,omitempty"`

	// memory current clock
	MemoryClock uint64 `json:"memory_clock,omitempty"`
}

// Sample data for a single job
//
// NOTE: Information about processes comes from various sources, and not all paths reveal all the
// information, hence there is some hedging about what values there can be, eg for user names.
//
// NOTE: The (job,epoch) pair must always be used together. If epoch is 0 then job is never 0 and
// other (job,0) records coming from the same or other nodes in the same cluster at the same or
// different time denote other aspects of the same job. Slurm jobs will have epoch=0, allowing us
// to merge event streams from the job both intra- and inter-node, while non-mergeable jobs will
// have epoch not zero. See extensive discussion in the "Rectification" section below.
//
// NOTE: Other job-wide / cross-process / per-slurm-job fields can be added, e.g. for I/O and
// energy, but only those that can only be monitored from within the node itself. Job data that can
// be extracted from a Slurm master node will be sent with the job data, see later.
//
// NOTE: Process entries can also be created for jobs running in containers. See below for
// comments about the data that can be collected.
//
// NOTE: On batch systems there may be more jobs than those managed by the batch system.
// These are distinguished by a non-zero epoch, see above.
type SampleJob struct {
	// The job ID
	Job uint64 `json:"job"`

	// User name on the cluster; `_user_<uid>` if not determined but user ID is available,
	// `_user_unknown` otherwise.
	User NonemptyString `json:"user"`

	// Zero for batch jobs, otherwise is a nonzero value that increases (by some amount) when the
	// system reboots, and never wraps around. You may think of it as a boot counter for the node,
	// but you must not assume that the values observed will be densely packed.  See notes.
	Epoch uint64 `json:"epoch"`

	// Processes in the job, all have the same Job ID.
	Processes []SampleProcess `json:"processes,omitempty"`
}

// Sample values for a single process within a job.
//
// NOTE: Other per-process fields can be added, eg for I/O and energy.
//
// NOTE: Memory utilization, produced by slurm-monitor, can be computed as resident_memory/memory
// where resident_memory is the field above and memory is that field in the sysinfo object for the
// node or in the slurm data (allocated memory).
//
// NOTE: Resident memory is a complicated figure. What we want is probably the Pss ("Proportional
// Set Size") which is private memory + a share of memory shared with other processes but that is
// often not available. Then we must choose from just private memory (RssAnon) or private memory +
// all resident memory shared with other processes (Rss). The former is problematic because it
// undercounts memory, the latter problematic because summing resident memory of the processes will
// frequently lead to a number that is more than physical memory as shared memory is counted
// several times.
//
// NOTE: Container software may not reveal all the process data we want. Docker, for example,
// provides cpu_util but not cpu_avg or cpu_time, and a memory utilization figure from which
// resident_memory must be back-computed.
//
// NOTE: The fields cpu_time, cpu_avg, and cpu_util are different views on the same
// quantities and are used variously by Sonar and the slurm-monitor dashboard. The Jobanalyzer
// back-end computes its own cpu_util from a time series of cpu_time values and using the
// cpu_avg as the first value in the computed series. The slurm-monitor dashboard in contrast
// uses cpu_util directly, but as it will require some time to perform the sampling it slows down
// the monitoring process (a little) and make it more expensive (a little), and the result is less
// accurate (it's a sample, not an averaging over the entire interval). Possibly having either
// cpu_avg and cpu_time together or cpu_util on its own would be sufficient.
//
// NOTE: `rolledup` is a Sonar data-compression feature that should probably be removed or
// improved, as information is lost. It is employed only if sonar is invoked with --rollup.  At the
// same time, for a node running 128 (say) MPI processes for the same job it represents a real
// savings in data volume.
type SampleProcess struct {
	// Kilobytes of private resident memory.
	ResidentMemory uint64 `json:"resident_memory,omitempty"`

	// Kilobytes of virtual data+stack memory
	VirtualMemory uint64 `json:"virtual_memory,omitempty"`

	// The command (not the command line), zombie processes get an extra <defunct> annotation at
	// the end, a la ps.
	Cmd string `json:"cmd,omitempty"`

	// Process ID, zero is used for rolled-up processes.
	Pid uint64 `json:"pid,omitempty"`

	// Parent-process ID.
	ParentPid uint64 `json:"ppid,omitempty"`

	// The running average CPU percentage over the true lifetime of the process as reported
	// by the operating system. 100.0 corresponds to "one full core's worth of computation".
	// See notes.
	CpuAvg float64 `json:"cpu_avg,omitempty"`

	// The current sampled CPU utilization of the process, 100.0 corresponds to "one full core's
	// worth of computation". See notes.
	CpuUtil float64 `json:"cpu_util,omitempty"`

	// Cumulative CPU time in seconds for the process over its lifetime. See notes.
	CpuTime uint64 `json:"cpu_time,omitempty"`

	// The number of additional processes in the same cmd and no child processes that have been
	// rolled into this one. That is, if the value is 1, the record represents the sum of the data
	// for two processes.
	Rolledup int `json:"rolledup,omitempty"`

	// GPU sample data for all cards used by the process.
	Gpus []SampleProcessGpu `json:"gpus,omitempty"`
}

// Per-process per-gpu sample data.
//
// NOTE: The difference between gpu_memory and gpu_memory_util is that, on some cards some of the
// time, it is possible to determine one of these but not the other, and vice versa. For example,
// on the NVIDIA cards we can read both quantities for running processes but only gpu_memory for
// some zombies. On the other hand, on our AMD cards there used to be no support for detecting the
// absolute amount of memory used, nor the total amount of memory on the cards, only the percentage
// of gpu memory used (gpu_memory_util). Sometimes we can convert one figure to another, but other
// times we cannot quite do that. Rather than encoding the logic for dealing with this in the
// monitoring component, the task is currently offloaded to the back end. It would be good to clean
// this up, with experience from more GPU types too - maybe gpu_memory_util can be removed.
//
// NOTE: Some cards do not reveal the amount of compute or memory per card per process, only which
// cards and how much compute or memory in aggregate (NVIDIA at least provides the more detailed
// data). In that case, the data revealed here for each card will be the aggregate figure for the
// process divided by the number of cards the process is running on.
type SampleProcessGpu struct {
	// Local card index, may change at boot
	Index uint64 `json:"index"`

	// Card UUID.  See preamble for notes about UUIDs.
	UUID NonemptyString `json:"uuid"`

	// The current GPU percentage utilization for the process on the card.
	//
	// (The "gpu_" prefix here and below is sort of redundant but have been retained since it makes
	// the fields analogous to the "cpu_" fields.)
	GpuUtil float64 `json:"gpu_util,omitempty"`

	// The current GPU memory used in kilobytes for the process on the card. See notes.
	GpuMemory uint64 `json:"gpu_memory,omitempty"`

	// The current GPU memory usage percentage for the process on the card. See notes.
	GpuMemoryUtil float64 `json:"gpu_memory_util,omitempty"`
}

// Jobs data are extracted from a single always-up master node on the cluster, and describe jobs
// under the control of a central jobs manager.
//
// NOTE: A stateful client can filter the data effectively.  Minimally it can filter against an
// in-memory database of job records and the state or fields that have been sent.  The backend must
// still be prepared to deal with redundant data as the client might crash and be resurrected, but
// we can still keep data volume down.
//
// JSON representation can be read with ConsumeJSONJobs().
type JobsEnvelope struct {
	// Information about the producer and data format
	Meta MetadataObject `json:"meta"`

	// Jobs data, for successful probes
	Data *JobsData `json:"data,omitempty"`

	// Error information, for unsuccessful probes
	Errors []ErrorObject `json:"errors,omitempty"`
}

// Jobs data, for successful jobs probes
type JobsData struct {
	// Data tag: The value "jobs"
	Type DataType `json:"type"`

	// The jobs data themselves
	Attributes JobsAttributes `json:"attributes"`
}

// A collection of jobs
//
// NOTE: There can eventually be other types of jobs, there will be other fields for them here, and
// the decoder will populate the correct field.  Other fields will be nil.
type JobsAttributes struct {
	// Time the current data were obtained
	Time Timestamp `json:"time"`

	// The canonical cluster name
	Cluster Hostname `json:"cluster"`

	// Individual job records.  There may be multiple records per job, one per job step.
	SlurmJobs []SlurmJob `json:"slurm_jobs,omitempty"`
}

// See extensive discussion in the postamble for what motivates the following spec.  In particular,
// Job IDs are very complicated.
//
// Fields below mostly carry names and semantics from the Slurm REST API, except where those names
// or semantics are unworkable.  (For example, the name field really needs to be job_name.)
//
// NOTE: Fields with substructure (AllocTRES, GRESDetail) may have parsers, see other files in this
// package.
//
// NOTE: There may be various ways of getting the data: sacct, scontrol, slurmrestd, or talking to
// the database directly.
//
// NOTE: References to "slurm" for the fields below are to the Slurm REST API specification.  That
// API is poorly documented and everything here is subject to change.
//
// NOTE: The first four fields, job_id, job_step, job_name, and job_state must be transmitted in
// every record.  Other fields depend on the nature and state of the job.  Every field should be
// transmitted with the first record for the step that is sent after the field acquires a value.
type SlurmJob struct {
	// The Slurm Job ID that directly controls the task that the record describes, in an array or
	// het job this is the primitive ID of the subjob.
	//
	// sacct: the part of `JobIDRaw` before the separator (".", "_", "+").
	//
	// slurm: `JOB_INFO.job_id`.
	JobID NonzeroUint `json:"job_id"`

	// The step identifier for the job identified by job_id.  For the topmost step/stage of a job
	// this will be the empty string.  Other values normally have the syntax of unsigned integers,
	// but may also be the strings "extern" and "batch".  This field's default value is the empty
	// string.
	//
	// NOTE: step 0 and step "empty string" are different, in fact thinking of a normal number-like
	// step name as a number may not be very helpful.
	//
	// sacct: the part of `JobIDRaw` after the separator.
	//
	// slurm: via jobs: `Job.STEP_STEP.name`, probably.
	JobStep string `json:"job_step,omitempty"`

	// The name of the job.
	//
	// sacct: `JobName`.
	//
	// slurm: `JOB_INFO.name`.
	JobName string `json:"job_name,omitempty"`

	// The state of the job described by the record, an all-uppercase word from the set PENDING,
	// RUNNING, CANCELLED, COMPLETED, DEADLINE, FAILED, OUT_OF_MEMORY, TIMEOUT.
	//
	// sacct: `State`, though sometimes there's additional information in the output that we will
	// discard ("CANCELLED by nnn").
	//
	// slurm: `JOB_STATE.current` probably, though that has multiple values.
	JobState NonemptyString `json:"job_state"`

	// The overarching ID of an array job, see discussion in the postamble.
	//
	// sacct: the n of a `JobID` of the form `n_m.s`
	//
	// slurm: `JOB_INFO.array_job_id`.
	ArrayJobID uint64 `json:"array_job_id,omitempty"`

	// if `array_job_id` is not zero, the array element's index.  Individual elements of an array
	// job have their own plain job_id; the `array_job_id` identifies these as part of the same array
	// job and the array_task_id identifies their position within the array, see later discussion.
	//
	// sacct: the m of a `JobID` of the form `n_m.s`.
	//
	// slurm: `JOB_INFO.array_task_id`.
	ArrayTaskID uint64 `json:"array_task_id,omitempty"`

	// If not zero, the overarching ID of a heterogenous job.
	//
	// sacct: the n of a `JobID` of the form `n+m.s`
	//
	// slurm: `JOB_INFO.het_job_id`
	HetJobID uint64 `json:"het_job_id,omitempty"`

	// If `het_job_id` is not zero, the het job element's index.
	//
	// sacct: the m of a `JobID` of the form `n+m.s`
	//
	// slurm: `JOB_INFO.het_job_offset`.
	HetJobOffset uint64 `json:"het_job_offset,omitempty"`

	// The name of the user running the job.  Important for tracking resources by user.
	//
	// sacct: `User`
	//
	// slurm: `JOB_INFO.user_name`
	UserName string `json:"user_name,omitempty"`

	// The name of the user's account.  Important for tracking resources by account.
	//
	// sacct: `Account`
	//
	// slurm: `JOB_INFO.account`
	Account string `json:"account,omitempty"`

	// The time the job was submitted.
	//
	// sacct: `Submit`
	//
	// slurm: `JOB_INFO.submit_time`
	SubmitTime Timestamp `json:"submit_time"`

	// The time limit in minutes for the job.
	//
	// sacct: `TimelimitRaw`
	//
	// slurm: `JOB_INFO.time_limit`
	Timelimit ExtendedUint `json:"time_limit,omitempty"`

	// The name of the partition to use.
	//
	// sacct: `Partition`
	//
	// slurm: `JOB_INFO.partiton`
	Partition string `json:"partition,omitempty"`

	// The name of the reservation.
	//
	// sacct: `Reservation`
	//
	// slurm: `JOB_INFO.resv_name`
	Reservation string `json:"reservation,omitempty"`

	// The nodes allocated to the job or step.
	//
	// sacct: `NodeList`
	//
	// slurm: `JOB_INFO.nodes`
	NodeList []string `json:"nodes,omitempty"`

	// The job priority.
	//
	// sacct: `Priority`
	//
	// slurm: `JOB_INFO.priority`
	Priority ExtendedUint `json:"priority,omitempty"`

	// Requested layout.
	//
	// sacct: `Layout`
	//
	// slurm: `JOB_INFO.steps[i].task.distribution`
	Layout string `json:"distribution,omitempty"`

	// Requested resources. For running jobs, the data can in part be synthesized from process
	// samples: we'll know the resources that are being used.
	//
	// sacct: TBD - TODO (possibly unavailable or maybe only when RUNNING).
	//
	// slurm: `JOB_INFO.gres_detail`
	GRESDetail []string `json:"gres_detail,omitempty"`

	// Number of requested CPUs.
	//
	// sacct: `ReqCPUS`
	//
	// slurm: `JOB_INFO.cpus`
	//
	// TODO: Is this per node?  If so, change the name of the field.
	ReqCPUS uint64 `json:"requested_cpus,omitempty"`

	// TODO: Description.  This may be the same as requested_cpus?
	//
	// sacct: TODO - TBD.
	//
	// slurm: `JOB_INFO.minimum_cpus_per_node`
	MinCPUSPerNode uint64 `json:"minimum_cpus_per_node,omitempty"`

	// Amount of requested memory.
	//
	// sacct: `ReqMem`
	//
	// slurm: `JOB_INFO.memory_per_node`
	ReqMemoryPerNode uint64 `json:"requested_memory_per_node,omitempty"`

	// Number of requested nodes.
	//
	// sacct: `ReqNodes`
	//
	// slurm: `JOB_INFO.node_count`
	ReqNodes uint64 `json:"requested_node_count,omitempty"`

	// Time the job started, if started
	//
	// sacct: `Start`
	//
	// slurm: `JOB_INFO.start_time`
	Start Timestamp `json:"start_time"`

	// Number of seconds the job was suspended
	//
	// sacct: `Suspended`
	//
	// slurm: `JOB_INFO.suspend_time`
	Suspended uint64 `json:"suspend_time,omitempty"`

	// Time the job ended (or was cancelled), if ended
	//
	// sacct: `End`
	//
	// slurm: `JOB_INFO.end_time`
	End Timestamp `json:"end_time"`

	// Job exit code, if ended
	//
	// sacct: `ExitCode`
	//
	// slurm: `JOB_INFO.exit_code.return_code`
	ExitCode uint64 `json:"exit_code,omitempty"`

	// Data specific to sacct output
	Sacct *SacctData `json:"sacct,omitempty"`
}

// SacctData are data aggregated by sacct and available if the sampling was done by sacct (as
// opposed to via the Slurm REST API).  The fields are named as they are in the sacct output, and
// the field documentation is mostly copied from the sacct man page.
type SacctData struct {
	// Minimum (system + user) CPU time of all tasks in job.
	MinCPU uint64 `json:"MinCPU,omitempty"`

	// Requested resources.  These are the resources allocated to the job/step after the job
	// started running.
	AllocTRES string `json:"AllocTRES,omitempty"`

	// Average (system + user) CPU time of all tasks in job.
	AveCPU uint64 `json:"AveCPU,omitempty"`

	// Average number of bytes read by all tasks in job.
	AveDiskRead uint64 `json:"AveDiskRead,omitempty"`

	// Average number of bytes written by all tasks in job.
	AveDiskWrite uint64 `json:"AveDiskWrite,omitempty"`

	// Average resident set size of all tasks in job.
	AveRSS uint64 `json:"AveRSS,omitempty"`

	// Average Virtual Memory size of all tasks in job.
	AveVMSize uint64 `json:"AveVMSize,omitempty"`

	// The job's elapsed time in seconds.
	ElapsedRaw uint64 `json:"ElapsedRaw,omitempty"`

	// The amount of system CPU time used by the job or job step.
	SystemCPU uint64 `json:"SystemCPU,omitempty"`

	// The amount of user CPU time used by the job or job step.
	UserCPU uint64 `json:"UserCPU,omitempty"`

	// Maximum resident set size of all tasks in job.
	MaxRSS uint64 `json:"MaxRSS,omitempty"`

	// Maximum Virtual Memory size of all tasks in job.
	MaxVMSize uint64 `json:"MaxVMSize,omitempty"`
}

// On clusters that have centralized cluster management (eg Slurm), the Cluster data reveal
// information about the cluster as a whole that are not derivable from data about individual nodes
// or jobs.
//
// JSON representation can be read with ConsumeJSONCluster().
type ClusterEnvelope struct {
	// Information about the producer and data format
	Meta MetadataObject `json:"meta"`

	// Node data, for successful probes
	Data *ClusterData `json:"data,omitempty"`

	// Error information, for unsuccessful probes
	Errors []ErrorObject `json:"errors,omitempty"`
}

// Cluster data, for successful cluster probes
type ClusterData struct {
	// Data tag: The value "cluster"
	Type DataType `json:"type"`

	// The cluster data themselves
	Attributes ClusterAttributes `json:"attributes"`
}

// Cluster description.
//
// NOTE: All clusters are assumed to have some unmanaged jobs.
type ClusterAttributes struct {
	// Time the current data were obtained
	Time Timestamp `json:"time"`

	// The canonical cluster name
	Cluster Hostname `json:"cluster"`

	// The `slurm` attribute is true if at least some nodes are under Slurm management.
	Slurm bool `json:"slurm,omitempty"`

	// Descriptions of the partitions on the cluster
	Partitions []ClusterPartition `json:"partitions,omitempty"`

	// Descriptions of the managed nodes on the cluster
	Nodes []ClusterNodes `json:"nodes,omitempty"`
}

// A Partition has a unique name and some nodes.  Nodes may be in multiple partitions.
type ClusterPartition struct {
	// Partition name
	Name NonemptyString `json:"name"`

	// Nodes in the partition
	Nodes []NodeRange `json:"nodes,omitempty"`
}

// A managed node is always on some state.  A node may be multiple states, in cluster-dependent
// ways (some of them really are "flags" on more general states); we expose as many as possible.
//
// NOTE: Node state depends on the cluster type.  For Slurm, see sinfo(1), it's a long list.
type ClusterNodes struct {
	// Constraint: The array of names may not be empty
	Names []NodeRange `json:"names,omitempty"`

	// The state(s) of the nodes in the range.  This is the output of sinfo as for the
	// StateComplete specifier, split into individual states, and the state names are always folded
	// to upper case.
	States []string `json:"states,omitempty"`
}

// A NodeRange is a nonempty-string representing a list of hostnames compactly using a simple
// syntax: brackets introduce a list of individual numbered nodes and ranges, these are expanded to
// yield a list of node names.  For example, `c[1-3,5]-[2-4].fox` yields `c1-2.fox`, `c1-3.fox`,
// `c1-4.fox`, `c2-2.fox`, `c2-3.fox`, `c2-4.fox`, `c3-2.fox`, `c3-3.fox`, `c3-4.fox`, `c5-2.fox`,
// `c5-3.fox`, `c5-4.fox`.  In a valid range, the first number is no greater than the second
// number, and numbers are not repeated.  (The motivation for this feature is that some clusters
// have very many nodes and that they group well this way.)
type NodeRange NonemptyString

// ## Slurm Job ID structure
//
// For an array job, the Job ID has the following structure.  When the job is submitted, the job is
// assigned an ID, call it J.  The task at each array index K then has the structure J_K.  However,
// that is for display purposes.  Underneath, each task is assigned an individual job ID T. My test
// job 1467073 with steps, 1, 3, 5, 7, have IDs that are displayed as 1467073_1, 1467073_3, and so
// on.  Importantly those jobs have underlying "true" IDs 1467074, 1467075, and so on (not
// necessarily densely packed, I expect).
//
// Within the job itself the SLURM_ARRAY_JOB_ID is 1467073 and the SLURM_ARRAY_TASK_ID is 1, 3, 5,
// 7, but in addition, the SLURM_JOB_ID is 1467074, 1467075, and so on.
//
// Each of the underlying jobs can themselves have steps.  So there is a record (exposed at least
// by sacct) that is called 1467073_1.extern, for that step.
//
// In the data above, we want the job_id to be the "true" underlying ID, the step to be the "true"
// job's step, and the array properties to additionally be set when it is an array job.  Hence for
// 1467073_1.extern we want job_id=1467074, jobs_step=extern, array_job_id=1467073, and
// array_task_id=1.  Here's how we can get that with sacct:
//
// ```
// |  $ sacct --user ec-larstha -P -o User,JobID,JobIDRaw
// |  User        JobID             JobIDRaw
// |  ec-larstha  1467073_1         1467074
// |              1467073_1.batch   1467074.batch
// |              1467073_1.extern  1467074.extern
// |              1467073_1.0       1467074.0
// |  ec-larstha  1467073_3         1467075
// |              1467073_3.batch   1467075.batch
// |              1467073_3.extern  1467075.extern
// |              1467073_3.0       1467075.0
// ```
//
// For heterogenous ("het") jobs, the situation is somewhat similar to array jobs, here's one with
// two groups:
//
// ```
// |  $ sacct --user ec-larstha -P -o User,JobID,JobIDRaw
// |  User        JobID               JobIDRaw
// |  ec-larstha  1467921+0           1467921
// |              1467921+0.batch     1467921.batch
// |              1467921+0.extern    1467921.extern
// |              1467921+0.0         1467921.0
// |  ec-larstha  1467921+1           1467922
// |              1467921+1.extern    1467922.extern
// |              1467921+1.0         1467922.0
// ```
//
// The second hetjob gets its own raw Job ID 1467922.  (Weirdly, so far as I've seen this ID is not
// properly exposed in the environment variables that the job sees.  Indeed there seems to be no
// way to distinguish from environment variables which hetgroup the job is in.  But this could have
// something to do with how I run the jobs, het jobs are fairly involved.)
//
// For jobs with job steps (multiple srun lines in the batch script but nothing else fancy), we get this:
//
// ```
// |  $ sacct --user ec-larstha -P -o User,JobID,JobIDRaw
// |  User        JobID           JobIDRaw
// |  ec-larstha  1470478         1470478
// |              1470478.batch   1470478.batch
// |              1470478.extern  1470478.extern
// |              1470478.0       1470478.0
// |              1470478.1       1470478.1
// |              1470478.2       1470478.2
// ```
//
// which is consistent with the previous cases, the step ID follows the . of the JobID or JobIDRaw.
//
// ## The meaning of a Job ID
//
// The job ID is complicated.
//
// We will assume that on a cluster where at least some jobs are controlled by Slurm there is a
// single Slurm instance that maintains an increasing job ID for all Slurm jobs and that job IDs
// are never recycled (I've heard from admins that things break if one does that, though see
// below).  Hence when a job is a Slurm job its job ID identifies any event stream belonging to the
// job, no matter when it was collected or what node it ran on, as belonging, and allows those
// streams to be merged into a coherent job view.  To do this, all we need to note in the job
// record is that it came from a Slurm job.
//
// However, there are non-Slurm jobs (even on Slurm clusters, due to the existence of non-Slurm
// nodes and due to other activity worthy of tracking even on Slurm nodes).  The job IDs for these
// jobs are collected from their process group IDs, per Posix, and the monitoring component will
// collect the data for the processes that belong to the same job on a given node into the same Job
// record.  On that node only, the data records for the same job make up the job's event stream.
// There are no inter-node jobs of this kind.  However, process group IDs may conflict with Slurm
// IDs and it is important to ensure that the records are not confused with each other.
//
// To make this more complicated still, non-Slurm IDs can be reused within a single node.  The
// reuse can happen over the long term, when the OS process IDs (and hence the process group IDs)
// wrap around, or over the short term, when a machine is rebooted, runs a job, then is rebooted
// again, runs another job, and so on (may be a typical pattern for a VM or container, or unstable
// nodes).
//
// The monitoring data expose job, epoch, node, and time.  The epoch is a representation of the
// node's boot time.  These fields work together as follows (for non-Slurm jobs).  Consider two
// records A and B. If A.job != B.job or A.node != B.node or A.epoch != B.epoch then they belong to
// different jobs.  Otherwise, we collect all records in which those three fields are the same and
// sort them by time.  If B follows A in the timeline and B.time - A.time > t then A and B are in
// different jobs (one ends with A and the next starts with B).
//
// A suitable value for t is probably on the order of a few hours, TBD.  Linux has a process ID
// space typically around 4e6. Some systems running very many jobs (though not usually HPC systems)
// can wrap around pids in a matter of days.  We want t to be shorter than the shortest plausible
// wraparound time.
type _postamble int
