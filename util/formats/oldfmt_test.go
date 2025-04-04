// Test the decoders for the old data format.  Currently we test only the dominant formats: JSON for
// sysinfo, CSV for samples and slurmjobs.

package formats

import (
	"os"
	"strings"
	"testing"

	"github.com/NordicHPC/sonar/util/formats/oldfmt"
)

func TestOldJSONSysinfo(t *testing.T) {
	f, err := os.Open("testdata/oldfmt_sysinfo.json")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	// There are two records in the file.
	var iter int
	err = oldfmt.ConsumeJSONSysinfo(f, func(info *oldfmt.SysinfoEnvelope) {
		switch iter {
		case 0:
			assert(t, info.Version == "0.13.100", "#0 version")
			assert(t, info.Timestamp == "2025-03-01T00:00:01+01:00", "#0 time")
			assert(t, info.Hostname == "ml1.hpc.uio.no", "#0 host")
			assert(t, strings.HasPrefix(info.Description, "2x14 (hyperthreaded) Intel(R) Xeon(R)"), "#0 desc")
			assert(t, info.CpuCores == 56, "#0 cores")
			assert(t, info.MemGB == 125, "#0 memory")
			assert(t, info.GpuCards == 3, "#0 gpu-cards")
			assert(t, info.GpuMemGB == 33, "#0 gpu-mem")
			assert(t, len(info.GpuInfo) == 3, "#0 gpu-info len")
			g := info.GpuInfo[1]
			assert(t, g.BusAddress == "00000000:3B:00.0", "#0 addr")
			assert(t, g.Index == 1, "#0 index")
			assert(t, g.UUID == "GPU-be013a01-364d-ca23-f871-206fe3f259ba", "#0 UUID")
			assert(t, g.Manufacturer == "NVIDIA", "#0 manufacturer")
			assert(t, g.Model == "NVIDIA GeForce RTX 2080 Ti", "#0 model")
			assert(t, g.Architecture == "Turing", "#0 arch")
			assert(t, g.Driver == "550.127.08", "#0 driver")
			assert(t, g.Firmware == "12.4", "#0 firmware")
			assert(t, g.MemKB == 11534336, "#0 card mem")
			assert(t, g.PowerLimit == 250, "#0 power limit")
			assert(t, g.MaxPowerLimit == 280, "#0 max power limit")
			assert(t, g.MinPowerLimit == 100, "#0 min power limit")
			assert(t, g.MaxCEClock == 2100, "#0 max ce clock")
			assert(t, g.MaxMemClock == 7000, "#0 max memory clock")
		case 1:
			assert(t, info.Version == "0.13.200", "#1 version")
			assert(t, info.Timestamp == "2025-02-28T00:00:01+01:00", "#1 time")
			// The rest tested adequately above
		}
		iter++
	})
	assert(t, iter == 2, "Iteration count")
	if err != nil {
		t.Fatal(err)
	}
}

func TestOldCSVSamples(t *testing.T) {
	f, err := os.Open("testdata/oldfmt_samples.csv")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	// There are four *groups* of records in the file.
	var iter int
	err = oldfmt.ConsumeCSVSamples(f, func(info *oldfmt.SampleEnvelope) {
		switch iter {
		case 0:
			assert(t, info.Timestamp == "2025-02-27T01:00:01+01:00", "#0 time")
			assert(t, info.Hostname == "c1-6.fox", "#0 host")
			assert(t, len(info.Samples) == 4, "#0 samples")
		case 1:
			assert(t, info.Timestamp == "2025-02-27T01:00:01+01:00", "#1 time")
			assert(t, info.Hostname == "gpu-11.fox", "#1 host")
			assert(t, len(info.Samples) == 8, "#1 samples")
		case 2:
			assert(t, info.Timestamp == "2025-02-27T01:05:01+01:00", "#2 time")
			assert(t, info.Hostname == "gpu-11.fox", "#2 host")
			assert(t, len(info.Samples) == 8, "#2 samples")
			s := info.Samples[3]
			assert(t, s.User == "ec-aad", "#2 user")
			assert(t, s.Cmd == "python", "#2 cmd")
			assert(t, s.JobId == 1345347, "#2 job")
			assert(t, s.Pid == 2164020, "#2 pid")
			assert(t, s.ParentPid == 2163996, "#2 ppid")
			assert(t, s.CpuPct == 898.6, "#2 cpu%")
			assert(t, s.CpuKib == 77012712, "#2 cpukib")
			assert(t, s.RssAnonKib == 70950544, "#2 rss")
			assert(t, s.Gpus == "3", "#2 gpus")
			assert(t, s.GpuPct == 47, "#2 gpu%")
			assert(t, s.GpuMemPct == 7, "#2 gpumem%")
			assert(t, s.GpuKib == 23418880, "#2 gpukib")
			assert(t, s.CpuTimeSec == 5369665, "#2 cputime")
		case 3:
			assert(t, len(info.CpuLoad) == 192, "#3 load len")
			// TODO: Test the contents of the array too
			assert(t, len(info.GpuSamples) == 8, "#3 gpu len")
			g := info.GpuSamples[2]
			assert(t, g.FanPct == 31, "#3 fan%")
			assert(t, g.PerfState == "P2", "#3 perfstate")
			assert(t, g.MemUse == 9629888, "#3 memuse")
			assert(t, g.CEUtilPct == 30, "#3 ce%")
			assert(t, g.MemUtilPct == 17, "#3 mem%")
			assert(t, g.Temp == 51, "#3 temp")
			assert(t, g.Power == 160, "#3 power")
			assert(t, g.PowerLimit == 350, "#3 powlim")
			assert(t, g.CEClock == 1695, "#3 ceclk")
			assert(t, g.MemClock == 9501, "#3 memclk")
		}
		iter++
	})
	assert(t, iter == 4, "Iteration count")
	if err != nil {
		t.Fatal(err)
	}
}

func TestOldCSVSlurmJobs(t *testing.T) {
	f, err := os.Open("testdata/oldfmt_slurmjobs.csv")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	// There are four *groups* of records in the file: good, good, error, good, and the first two
	// have the same JobID and are therefore separated.
	var iter int
	err = oldfmt.ConsumeCSVSlurmJobs(f, func(info any) {
		switch iter {
		case 0:
			e := info.(*oldfmt.SlurmEnvelope)
			assert(t, len(e.Jobs) == 5, "#0 len")
			assert(t, e.Jobs[0].JobID == "1382657", "#0 id")
		case 1:
			e := info.(*oldfmt.SlurmEnvelope)
			assert(t, len(e.Jobs) == 4, "#1 len")
			assert(t, e.Jobs[1].JobID == "1382657.batch", "#1 id")
		case 2:
			e := info.(*oldfmt.SlurmErrorEnvelope)
			assert(t, e.Error == "No can do", "#2 msg")
		case 3:
			e := info.(*oldfmt.SlurmEnvelope)
			assert(t, e.Jobs[0].JobID == "1368296", "#3 id")
		}
		iter++
	})
	assert(t, iter == 4, "Iteration count")
	if err != nil {
		t.Fatal(err)
	}
}
