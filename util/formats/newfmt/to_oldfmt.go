// The use case for this is transitional - when we receive new sysinfo but want to store it in old
// data files.  Note there is no old format for cluster data.
//
// The converters return either successful data or OldError error data.  The
// oldfmt.SlurmErrorEnvelope has a subset of the data for OldError, so just use OldError everywhere
// here.

package newfmt

import (
	"fmt"
	"math"
	"strings"

	"github.com/NordicHPC/sonar/util/formats/oldfmt"
)

type OldError struct {
	Version     string
	Timestamp   string
	Hostname    string
	Description string
}

func NewSampleToOld(d *SampleEnvelope) (*oldfmt.SampleEnvelope, *OldError) {
	if d.Errors != nil {
		e := d.Errors[0]
		return nil, &OldError{
			Version:     string(d.Meta.Version),
			Timestamp:   string(e.Time),
			Hostname:    string(e.Node),
			Description: string(e.Detail),
		}
	}

	o := new(oldfmt.SampleEnvelope)
	o.Version = string(d.Meta.Version)
	a := d.Data.Attributes
	o.Timestamp = string(a.Time)
	o.Hostname = string(a.Node)
	o.Cores = uint64(len(a.System.Cpus)) // Obsolete
	o.MemtotalKib = 0                    // Obsolete, luckily
	if len(a.System.Cpus) > 0 {
		cpuLoad := make([]uint64, len(a.System.Cpus))
		for i, n := range a.System.Cpus {
			cpuLoad[i] = uint64(n)
		}
		o.CpuLoad = cpuLoad
	}
	failing := make(map[uint64]uint64)
	if len(a.System.Gpus) > 0 {
		gpuLoad := make([]oldfmt.GpuSample, 0)
		for _, s := range a.System.Gpus {
			perf := "Unknown"
			if s.PerformanceState != ExtendedUintUnset {
				n, _ := s.PerformanceState.ToUint()
				perf = fmt.Sprintf("P%d", n)
			}
			failing[s.Index] = s.Failing
			gpuLoad = append(gpuLoad, oldfmt.GpuSample{
				FanPct:      s.Fan,
				ComputeMode: s.ComputeMode,
				PerfState:   perf,
				MemUse:      s.Memory,
				CEUtilPct:   s.CEUtil,
				MemUtilPct:  s.MemoryUtil,
				Temp:        uint64(max(0, s.Temperature)),
				Power:       s.Power,
				PowerLimit:  s.PowerLimit,
				CEClock:     s.CEClock,
				MemClock:    s.MemoryClock,
			})
		}
		o.GpuSamples = gpuLoad
	}
	if len(a.Jobs) > 0 {
		samples := make([]oldfmt.ProcessSample, 0)
		for _, job := range a.Jobs {
			for _, process := range job.Processes {
				var gpus []string
				var gpuPct float64
				var gpuMemPct float64
				var gpuKib uint64
				var gpuFail uint64
				for _, g := range process.Gpus {
					gpus = append(gpus, fmt.Sprint(g.Index))
					gpuPct += g.GpuUtil
					gpuMemPct += g.GpuMemoryUtil
					gpuKib += g.GpuMemory
					gpuFail |= failing[g.Index]
				}
				samples = append(samples, oldfmt.ProcessSample{
					User:       string(job.User),
					Cmd:        process.Cmd,
					JobId:      job.Job,
					Pid:        process.Pid,
					ParentPid:  process.ParentPid,
					CpuPct:     process.CpuAvg,
					CpuKib:     process.VirtualMemory,
					RssAnonKib: process.ResidentMemory,
					Gpus:       strings.Join(gpus, ","),
					GpuMemPct:  gpuMemPct,
					GpuKib:     gpuKib,
					CpuTimeSec: process.CpuTime,
					GpuFail:    gpuFail,
					Rolledup:   uint64(process.Rolledup),
				})
			}
		}
		o.Samples = samples
	}
	return o, nil
}

func NewSysinfoToOld(d *SysinfoEnvelope) (*oldfmt.SysinfoEnvelope, *OldError) {
	if d.Errors != nil {
		e := d.Errors[0]
		return nil, &OldError{
			Version:     string(d.Meta.Version),
			Timestamp:   string(e.Time),
			Hostname:    string(e.Node),
			Description: string(e.Detail),
		}
	}

	o := new(oldfmt.SysinfoEnvelope)
	o.Version = string(d.Meta.Version)
	a := d.Data.Attributes
	o.Timestamp = string(a.Time)
	o.Hostname = string(a.Node)
	o.CpuCores = uint64(a.Sockets * a.CoresPerSocket * a.ThreadsPerCore)
	o.MemGB = uint64(math.Ceil(float64(a.Memory) / (1024 * 1024)))
	cards := a.Cards
	if cards != nil {
		o.GpuCards = uint64(len(cards))
		var kb uint64
		for _, c := range cards {
			kb += c.Memory
		}
		o.GpuMemGB = uint64(math.Ceil(float64(kb) / (1024 * 1024)))
		gpus := make([]oldfmt.GpuSysinfo, len(cards))
		for i, c := range cards {
			gpus[i].BusAddress = c.Address
			gpus[i].Index = c.Index
			gpus[i].UUID = c.UUID
			gpus[i].Manufacturer = c.Manufacturer
			gpus[i].Model = c.Model
			gpus[i].Architecture = c.Architecture
			gpus[i].Driver = c.Driver
			gpus[i].Firmware = c.Firmware
			gpus[i].MemKB = c.Memory
			gpus[i].PowerLimit = c.PowerLimit
			gpus[i].MaxPowerLimit = c.MaxPowerLimit
			gpus[i].MinPowerLimit = c.MinPowerLimit
			gpus[i].MaxCEClock = c.MaxCEClock
			gpus[i].MaxMemClock = c.MaxMemoryClock
		}
		o.GpuInfo = gpus
	}
	var ht string
	if a.ThreadsPerCore > 1 {
		ht = " (hyperthreaded)"
	}
	var gpuDesc string
	var i int
	for i < len(cards) {
		first := i
		for i < len(cards) &&
			cards[i].Model == cards[first].Model &&
			cards[i].Memory == cards[first].Memory {
			i++
		}
		memsize := "unknown"
		if cards[first].Memory > 0 {
			memsize = fmt.Sprint(uint64(math.Ceil(float64(cards[first].Memory) / (1024 * 1024))))
		}
		gpuDesc += fmt.Sprintf(", %dx %s @ %dGiB", i-first, cards[first].Model, memsize)
	}
	o.Description = fmt.Sprintf("%dx%d%s %s, %d GiB%s",
		a.Sockets, a.CoresPerSocket, ht, a.CpuModel, o.MemGB, gpuDesc)
	return o, nil
}

func NewJobToOld(d *JobsEnvelope) (*oldfmt.SlurmEnvelope, *OldError) {
	if d.Errors != nil {
		e := d.Errors[0]
		return nil, &OldError{
			Version:     string(d.Meta.Version),
			Timestamp:   string(e.Time),
			Hostname:    string(e.Node),
			Description: string(e.Detail),
		}
	}

	o := new(oldfmt.SlurmEnvelope)
	o.Version = string(d.Meta.Version)
	jobs := make([]oldfmt.SlurmJob, 0)
	a := d.Data.Attributes
	dummySacct := new(SacctData)
	for _, job := range a.SlurmJobs {
		if job.JobState == "RUNNING" || job.JobState == "PENDING" {
			continue
		}

		var jobIDRaw string
		if job.JobStep != "" {
			jobIDRaw = fmt.Sprintf("%d.%s", job.JobID, job.JobStep)
		} else {
			jobIDRaw = fmt.Sprint(job.JobID)
		}

		var jobID string
		if job.ArrayJobID != 0 {
			// We want <jobid>_<taskid> or <jobid>_<taskid>.<step>
			if job.JobStep != "" {
				jobID = fmt.Sprintf("%d_%d.%s", job.ArrayJobID, job.ArrayTaskID, job.JobStep)
			} else {
				jobID = fmt.Sprintf("%d_%d", job.ArrayJobID, job.ArrayTaskID)
			}
		} else if job.HetJobID != 0 {
			if job.JobStep != "" {
				jobID = fmt.Sprintf("%d+%d.%s", job.HetJobID, job.HetJobOffset, job.JobStep)
			} else {
				jobID = fmt.Sprintf("%d+%d", job.HetJobID, job.HetJobOffset)
			}
		} else {
			// leave it blank
		}

		sacct := job.Sacct
		if sacct == nil {
			sacct = dummySacct
		}
		var timelimit string
		if job.Timelimit != ExtendedUintUnset {
			timelimit = fmt.Sprint(job.Timelimit.ToUint())
		}
		var priority string
		if job.Priority != ExtendedUintUnset {
			priority = fmt.Sprint(job.Priority.ToUint())
		}
		jobs = append(jobs, oldfmt.SlurmJob{
			JobID:        jobID,
			JobIDRaw:     jobIDRaw,
			User:         job.UserName,
			Account:      job.Account,
			State:        string(job.JobState),
			Start:        string(job.Start),
			End:          string(job.End),
			AveCPU:       fmt.Sprint(sacct.AveCPU),
			AveDiskRead:  fmt.Sprint(sacct.AveDiskRead),
			AveDiskWrite: fmt.Sprint(sacct.AveDiskWrite),
			AveRSS:       fmt.Sprint(sacct.AveRSS),
			AveVMSize:    fmt.Sprint(sacct.AveVMSize),
			ElapsedRaw:   fmt.Sprint(sacct.ElapsedRaw),
			ExitCode:     fmt.Sprint(job.ExitCode),
			Layout:       string(job.Layout),
			MaxRSS:       fmt.Sprint(sacct.MaxRSS),
			MaxVMSize:    fmt.Sprint(sacct.MaxVMSize),
			MinCPU:       fmt.Sprint(sacct.MinCPU),
			ReqCPUS:      fmt.Sprint(job.ReqCPUS),
			ReqMem:       fmt.Sprint(job.ReqMemoryPerNode),
			ReqNodes:     fmt.Sprint(job.ReqNodes),
			Reservation:  job.Reservation,
			Submit:       string(job.SubmitTime),
			Suspended:    fmt.Sprint(job.Suspended),
			SystemCPU:    fmt.Sprint(sacct.SystemCPU),
			TimelimitRaw: timelimit,
			UserCPU:      fmt.Sprint(sacct.UserCPU),
			NodeList:     strings.Join(job.NodeList, ","),
			Partition:    job.Partition,
			AllocTRES:    sacct.AllocTRES,
			Priority:     priority,
			JobName:      job.JobName,
		})
	}
	return o, nil
}
