// SPDX-License-Identifier: MIT

// The purpose of this test:
//
// - touch every defined data field at least once and make sure it has the expected value
// - check some internal consistency, eg Errors xor Data
// - check the TRES parser
// - check that every JSON field defined in newfmt/types.go is emitted by Rust code, and only
//   using their symbolic names
// - check the JSON consumer logic

package formats

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"reflect"
	"regexp"
	"strings"
	"testing"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
)

func TestNewJSONSysinfo(t *testing.T) {
	f, err := os.Open("testdata/newfmt_sysinfo.json")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	// There are three records: one for ml1 with GPUs, one for c1-6.fox without, one error
	var iter int
	err = newfmt.ConsumeJSONSysinfo(f, false, func(info *newfmt.SysinfoEnvelope) {
		switch iter {
		case 0:
			assert(t, info.Meta.Producer == "sonar", "#0 producer")
			assert(t, info.Meta.Version == "0.13.0", "#0 version")
			assert(t, info.Errors == nil, "#0 errors")
			assert(t, info.Data.Type == "sysinfo", "#0 type")
			a := info.Data.Attributes
			assert(t, a.Time == "2025-03-01T00:00:01+01:00", "#0 time")
			assert(t, a.Cluster == "mlx.hpc.uio.no", "#0 cluster")
			assert(t, a.Node == "ml1.hpc.uio.no", "#0 node")
			assert(t, a.OsName == "Linux", "#0 os_name")
			assert(t, a.OsRelease == "4.18.0-553.30.1.el8_10.x86_64", "#0 os_version")
			assert(t, a.Sockets == 2, "#0 sockets")
			assert(t, a.CoresPerSocket == 3, "#0 cores per socket")
			assert(t, a.ThreadsPerCore == 5, "#0 threads per core")
			assert(t, a.CpuModel == "yoyodyne-3", "#0 core model")
			assert(t, a.Memory == 131072000, "#0 memory")
			assert(t, len(a.Cards) == 3, "#0 cards")
			c := a.Cards[1]
			assert(t, c.Index == 1, "#0 card index")
			assert(t, c.UUID == "GPU-be013a01-364d-ca23-f871-206fe3f259ba", "#0 card UUID")
			assert(t, c.Address == "00000000:3B:00.0", "#0 card address")
			assert(t, c.Manufacturer == "NVIDIA", "#0 card manufacturer")
			assert(t, c.Model == "NVIDIA GeForce RTX 2080 Ti", "#0 card model")
			assert(t, c.Architecture == "Turing", "#0 card arch")
			assert(t, c.Driver == "550.127.08", "#0 card driver")
			assert(t, c.Firmware == "12.4", "#0 card firmware")
			assert(t, c.Memory == 11534336, "#0 card memory")
			assert(t, c.PowerLimit == 250, "#0 card power limit")
			assert(t, c.MaxPowerLimit == 280, "#0 card max power limit")
			assert(t, c.MinPowerLimit == 100, "#0 card min power limit")
			assert(t, c.MaxCEClock == 2100, "#0 card max ce clock")
			assert(t, c.MaxMemoryClock == 7000, "#0 card max memory clock")
		case 1:
			a := info.Data.Attributes
			assert(t, a.Cluster == "fox.educloud.no", "#1 cluster")
			assert(t, a.Node == "c1-6.fox", "#1 node")
		case 2:
			assert(t, info.Errors != nil, "#2 errors")
			assert(t, info.Errors[0].Detail == "Node not cooperating", "#2 msg")
		}
		iter++
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, iter == 3, "Iteration count")
}

// Test that unknown fields are caught in strict mode

func TestNewJSONSysinfo2(t *testing.T) {
	f := strings.NewReader(`{"zappa":"hello"}`)
	err := newfmt.ConsumeJSONSysinfo(f, true, func(info *newfmt.SysinfoEnvelope) {})
	assert(t, err != nil && strings.Index(err.Error(), "unknown field") != -1, "Unknown field #1 msg")

	f = strings.NewReader(`{"meta":{"zappa":"hello"}}`)
	err = newfmt.ConsumeJSONSysinfo(f, true, func(info *newfmt.SysinfoEnvelope) {})
	assert(t, err != nil && strings.Index(err.Error(), "unknown field") != -1, "Unknown field #2 msg")
}

func TestNewJSONSysinfoActual(t *testing.T) {
	// Do this:
	// - run `sonar sysinfo` once
	// - parse the output in strict mode
	//
	// This ensures that:
	// - all emitted, known fields have the right types
	// - no unknown fields are emitted
	//
	// Do this on enough machines and we'll have a decent test of whether Sonar works in practice.

	stdout := runSonar(t, "sysinfo", "--cluster", "xyzzy.no", "--json")
	err := newfmt.ConsumeJSONSysinfo(
		strings.NewReader(stdout),
		true,
		func(info *newfmt.SysinfoEnvelope) {},
	)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNewJSONSamples(t *testing.T) {
	f, err := os.Open("testdata/newfmt_samples.json")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var iter int
	err = newfmt.ConsumeJSONSamples(f, false, func(info *newfmt.SampleEnvelope) {
		switch iter {
		case 0:
			// lth laptop
			assert(t, info.Meta.Producer == "sonar", "#0 producer")
			assert(t, info.Meta.Version == "0.13.0", "#0 version")
			assert(t, info.Meta.Format == 37, "#0 format") // will bite us later, but not now
			assert(t, info.Meta.Token == "abc", "#0 token")
			assert(t, info.Errors == nil, "#0 errors")
			assert(t, info.Data.Type == "sample", "#0 type")
			a := info.Data.Attributes
			assert(t, a.Time == "2025-04-07T13:53:24+02:00", "#0 time")
			assert(t, a.Cluster == "bling.uio.no", "#0 cluster")
			assert(t, a.Node == "bling", "#0 node")
			s := a.System
			assert(t, len(s.Cpus) == 8, "#0 system cpus")
			assert(t, s.Cpus[2] == 808, "#0 system cpu time")
			assert(t, len(s.Gpus) == 0, "#0 system gpus")
			assert(t, s.UsedMemory == 5992528, "#0 system mem")
			assert(t, len(a.Jobs) == 3, "#0 jobs")
			j := a.Jobs[2]
			assert(t, j.Job == 2610, "#0 job id")
			assert(t, j.User == "larstha", "#0 user")
			assert(t, j.Epoch == 166179831, "#0 epoch")
			assert(t, len(j.Processes) == 1, "#0 processes")
			p := j.Processes[0]
			assert(t, p.ResidentMemory == 21284, "#0 resident")
			assert(t, p.VirtualMemory == 42516, "#0 virtual")
			assert(t, p.Cmd == "pipewire-pulse", "#0 Cmd")
			assert(t, p.Pid == 2610, "#0 pid")
			assert(t, p.ParentPid == 2569, "#0 ppid")
			assert(t, p.CpuAvg == 1.2, "#0 cpu avg")
			assert(t, p.CpuUtil == 0.1, "#0 cpu util")
			assert(t, p.CpuTime == 120, "#0 cpu time")
			assert(t, p.Read == 102, "#0 data read")
			assert(t, p.Written == 12, "#0 data written")
			assert(t, p.Cancelled == 7, "#0 data cancelled")
		case 1:
			// ml6
			a := info.Data.Attributes
			assert(t, a.Time == "2025-04-07T14:17:11+02:00", "#1 time")
			assert(t, a.Cluster == "mlx.hpc.uio.no", "#1 cluster")
			assert(t, a.Node == "ml6.hpc.uio.no", "#1 node")
			s := a.System
			assert(t, len(s.Cpus) == 64, "#1 system cpus")
			assert(t, s.Cpus[2] == 700787, "#1 system cpu time")
			assert(t, len(s.Gpus) == 8, "#1 system gpus")
			g := s.Gpus[2]
			assert(t, g.Index == 2, "#1 index")
			assert(t, g.UUID == "GPU-1a93320a-442c-ea70-48f0-13eec991b330", "#1 uuid")
			assert(t, g.Fan == 30, "#1 fan")
			assert(t, g.ComputeMode == "", "#1 compute mode")
			assert(t, g.PerformanceState == 3, "#1 perf state")
			assert(t, g.Memory == 4514240, "#1 memory")
			assert(t, g.CEUtil == 73, "#1 CE util")
			assert(t, g.MemoryUtil == 58, "#1 Memory util")
			assert(t, g.Temperature == 53, "#1 temperature")
			assert(t, g.Power == 146, "#1 power")
			assert(t, g.PowerLimit == 250, "#1 power limit")
			assert(t, g.CEClock == 1800, "#1 ce clock")
			assert(t, g.MemoryClock == 6800, "#1 memory clock")
		}
		iter++
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, iter == 2, "Iteration count")
}

func TestNewJSONSamplesActual(t *testing.T) {
	// See comments above about this logic
	stdout := runSonar(t, "ps", "--cluster", "xyzzy.no", "--json")
	err := newfmt.ConsumeJSONSamples(
		strings.NewReader(stdout),
		true,
		func(info *newfmt.SampleEnvelope) {},
	)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNewJSONCluster(t *testing.T) {
	f, err := os.Open("testdata/newfmt_cluster.json")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var iter int
	err = newfmt.ConsumeJSONCluster(f, false, func(info *newfmt.ClusterEnvelope) {
		switch iter {
		case 0:
			assert(t, info.Meta.Producer == "sonar", "#0 producer")
			assert(t, info.Meta.Version == "0.13.0", "#0 version")
			assert(t, info.Data.Type == "cluster", "#0 tag")
			assert(t, info.Errors == nil, "#0 errors")
			a := info.Data.Attributes
			assert(t, a.Time == "2025-04-01T12:41:18+02:00", "#0 time")
			assert(t, a.Slurm, "#0 slurm")
			assert(t, a.Cluster == "xyzzy.no", "#0 cluster")
			assert(t, a.Partitions[0].Name == "normal", "#0 partition name")
			assert(t, len(a.Partitions[0].Nodes) == 1, "#0 partition -> nodes len")
			assert(t, a.Partitions[0].Nodes[0] == "c1-[5-28]", "#0 partition -> node name")
			assert(t, a.Nodes[0].Names[0] == "c1-[12-13,16-18,27]", "#0 node names")
			assert(t, strings.Join(a.Nodes[0].States, ",") == "ALLOCATED,MAINTENANCE,RESERVED", "#0 states")
		case 1:
			a := info.Data.Attributes
			assert(t, a.Cluster == "fram.sigma2.no", "#1 cluster")
		}
		iter++
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, iter == 2, "Iteration count")
}

func TestNewJSONClusterActual(t *testing.T) {
	if p, _ := exec.LookPath("sinfo"); p == "" {
		return
	}
	// See comments above about this logic
	stdout := runSonar(t, "cluster", "--cluster", "xyzzy.no", "--json")
	err := newfmt.ConsumeJSONCluster(
		strings.NewReader(stdout),
		true,
		func(info *newfmt.ClusterEnvelope) {},
	)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNewJSONSlurmJobs(t *testing.T) {
	f, err := os.Open("testdata/newfmt_slurmjobs.json")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var iter int
	err = newfmt.ConsumeJSONJobs(f, false, func(info *newfmt.JobsEnvelope) {
		switch iter {
		case 0:
			assert(t, info.Errors == nil, "#0 defined")
			assert(t, info.Meta.Producer == "sonar", "#0 producer")
			assert(t, info.Meta.Version == "1.2.3", "#0 version")
			assert(t, info.Data.Type == "jobs", "#0 type")
			a := info.Data.Attributes
			assert(t, a.Cluster == "fox.educloud.no", "#0 cluster")
			assert(t, a.Time == "2025-03-11T09:31:00+01:00", "#0 time")
			assert(t, len(a.SlurmJobs) == 1, "#0 len")
			j := a.SlurmJobs[0]
			assert(t, j.JobID == 1, "#0 id")
			assert(t, j.JobStep == "0", "#0 step")
			assert(t, j.JobName == "zappa", "#0 name")
			assert(t, j.JobState == "COMPLETED", "#0 state")
			assert(t, j.ArrayJobID == 0, "#0 array id")
			assert(t, j.ArrayTaskID == 0, "#0 array task")
			assert(t, j.HetJobID == 0, "#0 het id")
			assert(t, j.HetJobOffset == 0, "#0 het offset")
			assert(t, j.UserName == "moonunit", "#0 user")
			assert(t, j.Account == "ec666", "#0 account")
			assert(t, j.SubmitTime == "2025-03-08T13:49:06+01:00", "#0 submit time")
			assert(t, j.Timelimit == 100, "#0 time limit")
			assert(t, j.Partition == "normal", "#0 partition")
			assert(t, j.Reservation == "big-cheese", "#0 reservation")
			assert(t, reflect.DeepEqual(j.NodeList, []string{"c1-[10-20]", "bigmem-1"}), "#0 nodelist")
			p, err := j.Priority.ToUint()
			if err != nil {
				t.Fatal(err)
			}
			assert(t, p == 1000, "#0 priority")
			assert(t, j.Layout == "cyclic", "#0 layout")
			assert(t, len(j.GRESDetail) == 0, "#0 gres")
			assert(t, j.ReqCPUS == 22, "#0 req cpus")
			assert(t, j.MinCPUSPerNode == 2, "#0 min cpus per node")
			assert(t, j.ReqMemoryPerNode == 12345678, "#0 req memory")
			assert(t, j.ReqNodes == 11, "#0 req nodes")
			assert(t, j.Start == "2025-03-08T18:11:02+01:00", "#0 start")
			assert(t, j.Suspended == 37, "#0 suspended")
			assert(t, j.End == "2025-03-10T01:35:24+01:00", "#0 end")
			assert(t, j.ExitCode == 1, "#0 exit")
			assert(t, j.Sacct.MinCPU == 5, "#0 MinCPU")
			assert(t, j.Sacct.AllocTRES == "billing=128,cpu=128,energy=52238802,mem=472.50G,node=1", "#0 alloctres")
			assert(t, j.Sacct.AveCPU == 5, "#0 AveCPU")
			assert(t, j.Sacct.AveDiskRead == 2000000, "#0 AveDiskRead")
			assert(t, j.Sacct.AveDiskWrite == 900000, "#0 AveDiskWrite")
			assert(t, j.Sacct.AveRSS == 64400000, "#0 AveRSS")
			assert(t, j.Sacct.AveVMSize == 400000, "#0 AveVMSize")
			assert(t, j.Sacct.ElapsedRaw == 113062, "#0 ElapsedRaw")
			assert(t, j.Sacct.SystemCPU == 12345, "#0 SystemCPU")
			assert(t, j.Sacct.UserCPU == 11111111, "#0 UserCPU")
			assert(t, j.Sacct.MaxRSS == 6444000, "#0 MaxRSS")
			assert(t, j.Sacct.MaxVMSize == 8000000, "#0 MaxVMSize")
			tres, dropped := newfmt.DecodeSlurmTRES(j.Sacct.AllocTRES)
			assert(t, len(dropped) == 0, "#0 tres dropped")
			assert(t, len(tres) == 5, "#0 tres len")
			assert(t, tres[3].Key == "mem", "#0 tres key")
			assert(t, tres[3].Value == 472.50*1024*1024*1024, "#0 tres val")
		case 1:
			assert(t, info.Errors != nil, "#2 error")
			e := info.Errors[0]
			assert(t, e.Detail == "No can do", "#2 msg")
			assert(t, e.Time == "2025-03-11T09:31:00+01:00", "#2 time")
		}
		iter++
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, iter == 2, "Iteration count")
}

func TestNewJSONSlurmJobs2(t *testing.T) {
	f := strings.NewReader(`{"zappa":"hello"}`)
	err := newfmt.ConsumeJSONJobs(f, true, func(info *newfmt.JobsEnvelope) {})
	assert(t, err != nil && strings.Index(err.Error(), "unknown field") != -1, "Unknown field #1 msg")

	f = strings.NewReader(`{"meta":{"zappa":"hello"}}`)
	err = newfmt.ConsumeJSONJobs(f, true, func(info *newfmt.JobsEnvelope) {})
	assert(t, err != nil && strings.Index(err.Error(), "unknown field") != -1, "Unknown field #2 msg")
}

func TestNewJSONSlurmJobsActual(t *testing.T) {
	if p, _ := exec.LookPath("sacct"); p == "" {
		return
	}
	// See comments above about this logic
	stdout := runSonar(t, "slurm", "--cluster", "xyzzy.no", "--json")
	err := newfmt.ConsumeJSONJobs(
		strings.NewReader(stdout),
		true,
		func(info *newfmt.JobsEnvelope) {},
	)
	if err != nil {
		t.Fatal(err)
	}
}

func TestDecodeSlurmTRES(t *testing.T) {
	xs, ys := newfmt.DecodeSlurmTRES("billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,zappa,node=1")
	assert(t, len(xs) == 6, "#0 kv len")
	assert(t, len(ys) == 1, "#0 dropped len")
	assert(t, ys[0] == "zappa", "#0 dropped")
	keys := []string{"billing", "cpu", "gres/gpu:rtx30", "gres/gpu", "mem", "node"}
	values := []any{int64(20), int64(20), int64(1), int64(1), int64(50 * 1024 * 1024 * 1024), int64(1)}
	for i := range keys {
		assert(t, xs[i].Key == keys[i], "#0 key")
		assert(t, xs[i].Value == values[i], "#0 value")
	}
}

var built bool

func runSonar(t *testing.T, args ...string) string {
	if !built {
		err := exec.Command("sh", "-c", "cd ../.. ; cargo build").Run()
		if err != nil {
			t.Fatal("Compiling sonar:", err)
		}
		built = true
	}
	cmdline := "../../target/debug/sonar " + strings.Join(args, " ")
	cmd := exec.Command("sh", "-c", cmdline)
	stdout, err := cmd.Output()
	if err != nil {
		t.Fatal("Running sonar:", err)
	}
	return string(stdout)
}

// This looks for the names defined in src/json_tags.rs and checks that they are used by some code
// in a subset of files in the source directory.  This is one check on whether the output code uses
// only well-defined strings.

var (
	defRe = regexp.MustCompile(`pub\s+const\s+([^\s]+)\s*:`)
	idRe  = regexp.MustCompile(`[A-Z][A-Z0-9_]*`)
)

func TestFieldNames1(t *testing.T) {
	f, err := os.Open("../../src/json_tags.rs")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	b := bufio.NewScanner(f)
	var ln int
	defined := make(map[string]int)
	for b.Scan() {
		ln++
		l := b.Text()
		if m := defRe.FindStringSubmatch(l); m != nil {
			if defined[m[1]] > 0 {
				panic("Multiple defs " + m[1])
			}
			defined[m[1]] = ln
		}
	}
	for _, fn := range []string{
		"../../src/cluster.rs",
		"../../src/sysinfo.rs",
		"../../src/output.rs",
		"../../src/ps.rs",
		"../../src/ps_newfmt.rs",
		"../../src/slurmjobs.rs",
	} {
		f, err := os.Open(fn)
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		b := bufio.NewScanner(f)
		var ln int
		for b.Scan() {
			ln++
			l := b.Text()
			for _, id := range idRe.FindAllString(l, -1) {
				delete(defined, id)
			}
		}
	}
	for k, v := range defined {
		fmt.Println(k, " ", v)
	}
	if len(defined) > 0 {
		t.Fatal("oops")
	}
}

// Extract all strings in some contexts from some Rust source files and ensure that there are none.
// This is a second check on whether the output code uses only well-defined names.
//
// In the input files, lines between //+ignore-strings and //-ignore-strings are ignored.

func TestFieldNames2(t *testing.T) {
	push_re := regexp.MustCompile(`push_(?:string|uint_full|uint|duration|date|volume|s|i|u|f|o|a|b)\([^"]*"([a-zA-Z0-9_-]*)`)
	var fail bool
	for _, fn := range []string{
		"../../src/cluster.rs",
		"../../src/sysinfo.rs",
		"../../src/output.rs",
		"../../src/ps.rs",
		"../../src/ps_newfmt.rs",
		"../../src/slurmjobs.rs",
	} {
		f, err := os.Open(fn)
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		b := bufio.NewScanner(f)
		var ln int
		var ignore bool
		for b.Scan() {
			ln++
			l := b.Text()
			if strings.Index(l, "//+ignore-strings") != -1 {
				ignore = true
				continue
			}
			if strings.Index(l, "//-ignore-strings") != -1 {
				ignore = false
				continue
			}
			if !ignore {
				if r := push_re.FindStringSubmatch(l); r != nil {
					if r[1] != "" {
						fmt.Printf("%s:%d: String found: %s\n", fn, ln, r[1])
						fail = true
					}
				}
			}
		}
	}
	if fail {
		t.Fatal("failed")
	}
}
