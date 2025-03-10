// JSON and CSV field names were always the same.  Any comments here are meant to be informative,
// the authoritative documentation is still doc/DATA-FORMAT.md.
//
// Number representations are chosen to be compatible with the new data format: invariably 64-bit
// integers and floats, and unsigned except where a sign is explicitly possible.

package oldfmt

// CSV decoding not implemented (was never used in the field) but is easy to create.  JSON
// representation can be read with ConsumeJSONSysinfo().

type SysinfoEnvelope struct {
	Version     string       `json:"version"`
	Timestamp   string       `json:"timestamp"`
	Hostname    string       `json:"hostname"`
	Description string       `json:"description"`
	CpuCores    uint64       `json:"cpu_cores"`
	MemGB       uint64       `json:"mem_gb"`
	GpuCards    uint64       `json:"gpu_cards"`
	GpuMemGB    uint64       `json:"gpumem_gb"`
	GpuInfo     []GpuSysinfo `json:"gpu_info"`
}

type GpuSysinfo struct {
	BusAddress    string `json:"bus_addr"`
	Index         uint64 `json:"index"`
	UUID          string `json:"uuid"`
	Manufacturer  string `json:"manufacturer"`
	Model         string `json:"model"`
	Architecture  string `json:"arch"`
	Driver        string `json:"driver"`
	Firmware      string `json:"firmware"`
	MemKB         uint64 `json:"mem_size_kib"`
	PowerLimit    uint64 `json:"power_limit_watt"`
	MaxPowerLimit uint64 `json:"max_power_limit_watt"`
	MinPowerLimit uint64 `json:"min_power_limit_watt"`
	MaxCEClock    uint64 `json:"max_ce_clock_mhz"`
	MaxMemClock   uint64 `json:"max_mem_clock_mhz"`
}

// CSV representation can be read with ConsumeCSVSamples().  JSON representation can be read with
// ConsumeJSONSamples().

type SampleEnvelope struct {
	Version     string          `json:"v"`
	Timestamp   string          `json:"time"`
	Hostname    string          `json:"host"`
	CpuLoad     []uint64        `json:"load"`
	GpuSamples  []GpuSample     `json:"gpuinfo"`
	Samples     []ProcessSample `json:"samples"`
	Cores       uint64          // Only in fairly old CSV data; obsolete
	MemtotalKib uint64          // Ditto
}

type GpuSample struct {
	FanPct      uint64 `json:"fan%"`
	ComputeMode string `json:"mode"` // Platform-dependent string
	PerfState   string `json:"perf"` // Pn for nonnegative n, or "Unknown"
	MemUse      uint64 `json:"musekib"`
	CEUtilPct   uint64 `json:"cutil%"`
	MemUtilPct  uint64 `json:"mutil%"`
	Temp        uint64 `json:"tempc"`
	Power       uint64 `json:"poww"`
	PowerLimit  uint64 `json:"powlimw"`
	CEClock     uint64 `json:"cez"`
	MemClock    uint64 `json:"memz"`
}

type ProcessSample struct {
	User       string  `json:"user"`
	Cmd        string  `json:"cmd"`
	JobId      uint64  `json:"job"`
	Pid        uint64  `json:"pid"`
	ParentPid  uint64  `json:"ppid"`
	CpuPct     float64 `json:"cpu%"`
	CpuKib     uint64  `json:"cpukib"`
	RssAnonKib uint64  `json:"rssanonkib"`
	Gpus       string  `json:"gpus"` // Decode further with DecodeGpusList()
	GpuPct     float64 `json:"gpu%"`
	GpuMemPct  float64 `json:"gpumem%"`
	GpuKib     uint64  `json:"gpukib"`
	CpuTimeSec uint64  `json:"cputime_sec"`
	GpuFail    uint64  `json:"gpufail"`
	Rolledup   uint64  `json:"rolledup"`
}

// JSON representation can be read with ConsumeJSONSlurmJobs().  CSV representation can be read with
// ConsumeCSVSlurmJobs().
//
// The Comsume*SlurmJobs consumers will provide either a SlurmEnvelope or a SlurmErrorEnvelope, as
// appropriate.  (The input object should have either `Jobs` or it should have both `Timestamp` and
// `Error`.)

type SlurmEnvelope struct {
	Version string     `json:"version"`
	Jobs    []SlurmJob `json:"jobs"`
}

type SlurmErrorEnvelope struct {
	Version   string `json:"version"`
	Timestamp string `json:"timestamp"`
	Error     string `json:"error"`
}

// Data fields are always strings, an unhappy accident resulting from the initial CSV encoding
// probably.

type SlurmJob struct {
	JobID        string `json:"JobID"`
	JobIDRaw     string `json:"JobIDRaw"`
	User         string `json:"User"`
	Account      string `json:"Account"`
	State        string `json:"State"`
	Start        string `json:"Start"`
	End          string `json:"End"`
	AveCPU       string `json:"AveCPU"`
	AveDiskRead  string `json:"AveDiskRead"`
	AveDiskWrite string `json:"AveDiskWrite"`
	AveRSS       string `json:"AveRSS"`
	AveVMSize    string `json:"AveVMSize"`
	ElapsedRaw   string `json:"ElapsedRaw"`
	ExitCode     string `json:"ExitCode"`
	Layout       string `json:"Layout"`
	MaxRSS       string `json:"MaxRSS"`
	MaxVMSize    string `json:"MaxVMSize"`
	MinCPU       string `json:"MinCPU"`
	ReqCPUS      string `json:"ReqCPUS"`
	ReqMem       string `json:"ReqMem"`
	ReqNodes     string `json:"ReqNodes"`
	Reservation  string `json:"Reservation"`
	Submit       string `json:"Submit"`
	Suspended    string `json:"Suspended"`
	SystemCPU    string `json:"SystemCPU"`
	TimelimitRaw string `json:"TimelimitRaw"`
	UserCPU      string `json:"UserCPU"`
	NodeList     string `json:"NodeList"`
	Partition    string `json:"Partition"`
	AllocTRES    string `json:"AllocTRES"`
	Priority     string `json:"Priority"`
	JobName      string `json:"JobName"`
}
