// New Sonar JSON data format (we no longer support any kind of CSV variant).  MetadataObject and
// ErrorObject are shared between the various data types, everything else is specific to the data
// type and has a name that clearly indicates that.  The top-level object for each data type is the
// "...Envelope" object.  Within each envelope, `Data` and `Errors` are exclusive of each other.
//
// Some fields present in the older Sonar data are no longer here, having been deemed redundant or
// obsolete.  Some are here in a different form.  Therefore, while old and new data are broadly
// compatible, there may be some minor problems translating between them.
//
// The exact semantics of fields are still defined in the spec in doc/DATA-FORMATS.md.

package newfmt

import (
	"errors"
)

// RFC3999 localtime+TZO with no sub-second precision: yyyy-mm-ddThh:mm:ss+hh:mm, "Z" for +00:00.
type Timestamp string

// Dotted host name or prefix of same, with standard restrictions on character set.
type Hostname string

// An unsigned value that carries two additional values: unset and infinite.  It has a more limited
// value range than regular unsigned.
type Xint int64

const (
	XintUnset    int64 = 0
	XintInfinite int64 = -1
)

func (e Xint) ToUint() (uint64, error) {
	if e > 0 {
		return uint64(e - 1), nil
	}
	return 0, errors.New("Not a numeric value")
}

// Enum tags for our record types
type DataType string

const (
	DTSample  DataType = "sample"
	DTSysinfo DataType = "sysinfo"
	DTJobs    DataType = "job"
	DTCluster DataType = "cluster"
)

type MetadataObject struct {
	Producer string `json:"producer"`
	Version  string `json:"version"`
	Format   uint64 `json:"format"`
	// The `Token` is not documented in the spec.  It is a makeshift authorization token (API token)
	// to be used with Envelope.Data.Attributes.Cluster, it proves that the producer of the datum
	// was authorized to produce data for that cluster name.
	Token string `json:"token"`
}

type ErrorObject struct {
	Time    Timestamp `json:"time"`
	Detail  string    `json:"detail"`
	Cluster Hostname  `json:"cluster"`
	Node    Hostname  `json:"node"`
}

type KVPair struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}

// JSON representation can be read with ConsumeJSONSysinfo().

type SysinfoEnvelope struct {
	Data   *SysinfoData   `json:"data"`
	Errors []ErrorObject  `json:"errors"`
	Meta   MetadataObject `json:"meta"`
}

type SysinfoData struct {
	Type       DataType          `json:"type"` // DTSysinfo
	Attributes SysinfoAttributes `json:"attributes"`
}

type SysinfoAttributes struct {
	Time           Timestamp                `json:"time"`
	Cluster        Hostname                 `json:"cluster"`
	Node           Hostname                 `json:"node"`
	OsName         string                   `json:"os_name"`
	OsRelease      string                   `json:"os_release"`
	Architecture   string                   `json:"architecture"`
	Sockets        uint64                   `json:"sockets"`
	CoresPerSocket uint64                   `json:"cores_per_socket"`
	ThreadsPerCore uint64                   `json:"threads_per_core"`
	CpuModel       string                   `json:"cpu_model"`
	Memory         uint64                   `json:"memory"`
	TopoSVG        string                   `json:"topo_svg"` // Base64-encoded SVG
	Cards          []SysinfoGpuCard         `json:"cards"`
	Software       []SysinfoSoftwareVersion `json:"software"`
}

type SysinfoGpuCard struct {
	Index         uint64 `json:"index"`
	UUID          string `json:"uuid"`
	Address       string `json:"address"`
	Manufacturer  string `json:"manufacturer"`
	Model         string `json:"model"`
	Architecture  string `json:"architecture"`
	Driver        string `json:"driver"`
	Firmware      string `json:"firmware"`
	Memory        uint64 `json:"memory"`
	PowerLimit    uint64 `json:"power_limit"`
	MaxPowerLimit uint64 `json:"max_power_limit"`
	MinPowerLimit uint64 `json:"min_power_limit"`
	MaxCEClock    uint64 `json:"max_ce_clock"`
	MaxMemClock   uint64 `json:"max_memory_clock"`
}

type SysinfoSoftwareVersion struct {
	Key     string `json:"key"`
	Name    string `json:"name"`
	Version string `json:"version"`
}

// JSON representation can be read with ConsumeJSONSamples().

type SampleEnvelope struct {
	Data   *SampleData    `json:"data"`
	Errors []ErrorObject  `json:"errors"`
	Meta   MetadataObject `json:"meta"`
}

type SampleData struct {
	Type       DataType         `json:"type"` // DTSample
	Attributes SampleAttributes `json:"attributes"`
}

type SampleAttributes struct {
	Time    Timestamp     `json:"time"`
	Cluster Hostname      `json:"cluster"`
	Node    Hostname      `json:"node"`
	Attrs   []KVPair      `json:"attrs"`
	System  SampleSystem  `json:"system"`
	Jobs    []SampleJob   `json:"jobs"`
	Errors  []ErrorObject `json:"errors"`
}

type SampleSystem struct {
	Cpus       []SampleCpu `json:"cpus"`
	Gpus       []SampleGpu `json:"gpus"`
	UsedMemory uint64      `json:"used_memory"`
}

type SampleCpu uint64

type SampleGpu struct {
	Index            uint64 `json:"index"`
	UUID             string `json:"uuid"`
	Failing          uint64 `json:"failing"`
	Fan              uint64 `json:"fan"`
	ComputeMode      string `json:"compute_mode"`
	PerformanceState Xint   `json:"performance_state"`
	Memory           uint64 `json:"memory"`
	CEUtil           uint64 `json:"ce_util"`
	MemUtil          uint64 `json:"memory_util"`
	Temperature      int64  `json:"temperature"`
	Power            uint64 `json:"power"`
	PowerLimit       uint64 `json:"power_limit"`
	CEClock          uint64 `json:"ce_clock"`
	MemClock         uint64 `json:"memory_clock"`
}

type SampleJob struct {
	Job       uint64          `json:"job"`
	User      string          `json:"user"`
	Epoch     uint64          `json:"epoch"`
	Processes []SampleProcess `json:"processes"`
}

type SampleProcess struct {
	Resident  uint64             `json:"resident_memory"`
	Virtual   uint64             `json:"virtual_memory"`
	Cmd       string             `json:"cmd"`
	Pid       uint64             `json:"pid"`
	ParentPid uint64             `json:"ppid"`
	CpuAvg    float64            `json:"cpu_avg"`
	CpuUtil   float64            `json:"cpu_util"`
	CpuTime   uint64             `json:"cpu_time"`
	Rolledup  int                `json:"rolledup"`
	Gpus      []SampleProcessGpu `json:"gpus"`
}

type SampleProcessGpu struct {
	UUID       string  `json:"uuid"`
	GpuUtil    float64 `json:"gpu_util"`
	GpuMem     uint64  `json:"gpu_memory"`
	GpuMemUtil float64 `json:"gpu_memory_util"`
}

// JSON representation can be read with ConsumeJSONJobs().

type JobsEnvelope struct {
	Data   *JobsData      `json:"data"`
	Errors []ErrorObject  `json:"errors"`
	Meta   MetadataObject `json:"meta"`
}

type JobsData struct {
	Type       DataType       `json:"type"` // DTJobs
	Attributes JobsAttributes `json:"attributes"`
}

type JobsAttributes struct {
	Time    Timestamp `json:"time"`
	Cluster Hostname  `json:"cluster"`
	// There can eventually be other types of jobs, there will be other fields for them here, and
	// the decoder will populate the correct field.  Other fields will be nil.
	SlurmJobs []SlurmJob `json:"slurm_jobs"`
}

// This follows the order of the spec (at the time I write this).  Fields with substructure
// (AllocTRES, GRESDetail) may have parsers, see other files in this package.

type SlurmJob struct {
	JobID          uint64    `json:"job_id"`
	JobName        string    `json:"job_name"`
	JobState       string    `json:"job_state"`
	JobStep        string    `json:"job_step"`
	ArrayJobID     uint64    `json:"array_job_id"`
	ArrayTaskID    uint64    `json:"array_task_id"`
	HetJobID       uint64    `json:"het_job_id"`
	HetJobOffset   uint64    `json:"het_job_offset"`
	UserName       string    `json:"user_name"`
	Account        string    `json:"account"`
	SubmitTime     Timestamp `json:"submit_time"`
	Timelimit      Xint      `json:"time_limit"`
	Partition      string    `json:"partition"`
	Reservation    string    `json:"reservation"`
	NodeList       []string  `json:"nodes"`
	Priority       Xint      `json:"priority"`
	Layout         string    `json:"distribution"`
	GRESDetail     []string  `json:"gres_detail"`
	ReqCPUS        uint64    `json:"requested_cpus"`
	ReqMem         uint64    `json:"requested_memory_per_node"`
	ReqNodes       uint64    `json:"requested_node_count"`
	MinCPUSPerNode uint64    `json:"minimum_cpus_per_node"`
	Start          Timestamp `json:"start_time"`
	Suspended      uint64    `json:"suspend_time"`
	End            Timestamp `json:"end_time"`
	ExitCode       uint64    `json:"exit_code"`
	Sacct          *SacctData `json:"sacct"`
}

type SacctData struct {
	MinCPU       uint64 `json:"MinCPU"`
	AllocTRES    string `json:"AllocTRES"`
	AveCPU       uint64 `json:"AveCPU"`
	AveDiskRead  uint64 `json:"AveDiskRead"`
	AveDiskWrite uint64 `json:"AveDiskWrite"`
	AveRSS       uint64 `json:"AveRSS"`
	AveVMSize    uint64 `json:"AveVMSize"`
	ElapsedRaw   uint64 `json:"ElapsedRaw"`
	SystemCPU    uint64 `json:"SystemCPU"`
	UserCPU      uint64 `json:"UserCPU"`
	MaxRSS       uint64 `json:"MaxRSS"`
	MaxVMSize    uint64 `json:"MaxVMSize"`
}

// JSON representation can be read with ConsumeJSONCluster().

type ClusterEnvelope struct {
	Data   *ClusterData   `json:"data"`
	Errors []ErrorObject  `json:"errors"`
	Meta   MetadataObject `json:"meta"`
}

type ClusterData struct {
	Type       DataType          `json:"type"` // DTCluster
	Attributes ClusterAttributes `json:"attributes"`
}

// `Slurm` is set if at least some nodes and jobs are managed by Slurm.  All clusters are assumed to
// have some unmanaged jobs.

type ClusterAttributes struct {
	Time       Timestamp          `json:"time"`
	Cluster    Hostname           `json:"cluster"`
    Slurm      bool               `json:"slurm"`
	Partitions []ClusterPartition `json:"partitions"`
	Nodes      []ClusterNodes     `json:"nodes"`
}

type ClusterPartition struct {
	Name  string      `json:"name"`
	Nodes []NodeRange `json:"nodes"`
}

// Node state depends on the cluster type.  For Slurm, see sinfo(1), it's a long list.

type ClusterNodes struct {
	Names  []NodeRange `json:"names"`
	States []string    `json:"states"`
}

// Bracket-compressed node list element.

type NodeRange string
