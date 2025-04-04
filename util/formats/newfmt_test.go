// The purpose of this test:
//
// - touch every defined data field at least once and make sure it has the expected value
// - check some internal consistency, eg Errors xor Data
// - check the TRES parser
// - check that every JSON field defined in newfmt/types.go is emitted by Rust code, and vice versa
// - (less important) check the JSON consumer logic

package formats

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
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
			assert(t, c.MaxMemClock == 7000, "#0 card max memory clock")
			assert(t, len(a.Software) == 0, "#0 software")
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
		func(info *newfmt.SysinfoEnvelope) { },
	)
	if err != nil {
		t.Fatal(err)
	}
}

// TODO: Samples all fields

func TestNewJSONSamplesActual(t *testing.T) {
	// See comments above about this logic
	stdout := runSonar(t, "ps", "--cluster", "xyzzy.no", "--json")
	err := newfmt.ConsumeJSONSamples(
		strings.NewReader(stdout),
		true,
		func(info *newfmt.SampleEnvelope) { },
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
			// TODO: All fields
			assert(t, info.Meta.Producer == "sonar", "#0 producer")
			assert(t, info.Data.Type == "cluster", "#0 tag")
			assert(t, info.Errors == nil, "#0 errors")
			a := info.Data.Attributes
			assert(t, a.Slurm, "#0 slurm")
			assert(t, a.Cluster == "xyzzy.no", "#0 cluster")
			assert(t, a.Partitions[0].Name == "normal", "#0 partition name")
			assert(t, len(a.Partitions[0].Nodes) == 1, "#0 nodes len")
			assert(t, a.Partitions[0].Nodes[0] == "c1-[5-28]", "#0 node name")
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
		func(info *newfmt.ClusterEnvelope) { },
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
			// TODO: All fields
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
		func(info *newfmt.JobsEnvelope) { },
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

// This is a hack but it works well.  Extract all strings in some contexts from some Rust source
// files and then all json fields from types.go and make sure the sets are the same.  I've found a
// bunch of bugs with this.  It basically ensures that we don't change one piece of code without
// changing the other.  (Clearly the final step is to sync both to the documentation.)
//
// Lines between //+oldnames and //-oldnames are ignored.  Yay #ifdef.
//
// All strings on each line found between //+implicit-use and //-implicit-use are marked as used.

func TestFieldNames(t *testing.T) {
	push_re := regexp.MustCompile(`push_(?:string|uint_full|uint|duration|date|volume|s|i|u|f|o|a|b)\([^"]*"([a-zA-Z0-9_-]*)`)
	implicit_re := regexp.MustCompile(`"([a-zA-Z0-9_-]*)"`)
	usedStrings := make(map[string]string)
	defdStrings := make(map[string]int)
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
		var oldnames bool
		var implicitUse bool
		for b.Scan() {
			ln++
			l := b.Text()
			if strings.Index(l, "//+oldnames") != -1 {
				oldnames = true
				continue
			}
			if strings.Index(l, "//-oldnames") != -1 {
				oldnames = false
				continue
			}
			if strings.Index(l, "//+ignore-strings") != -1 {
				oldnames = true
				continue
			}
			if strings.Index(l, "//-ignore-strings") != -1 {
				oldnames = false
				continue
			}
			if strings.Index(l, "//+implicit-use") != -1 {
				implicitUse = true
				continue
			}
			if strings.Index(l, "//-implicit-use") != -1 {
				implicitUse = false
				continue
			}
			if !oldnames {
				if r := push_re.FindStringSubmatch(l); r != nil {
					if r[1] != "" {
						usedStrings[r[1]] = fmt.Sprint(fn, ":", ln)
					}
				}
			}
			if implicitUse {
				if r := implicit_re.FindAllStringSubmatch(l, -1); r != nil {
					for _, m := range r {
						usedStrings[m[1]] = fmt.Sprint(fn, ":", ln)
					}
				}
			}
		}
	}
	json_re := regexp.MustCompile(`json:"([a-zA-Z0-0_-]*)`)
	{
		f, err := os.Open("newfmt/types.go")
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		b := bufio.NewScanner(f)
		var ln int
		for b.Scan() {
			ln++
			l := b.Text()
			if r := json_re.FindStringSubmatch(l); r != nil {
				defdStrings[r[1]] = ln
			}
		}
	}
	var fail bool
	for k, v := range usedStrings {
		if defdStrings[k] == 0 {
			fail = true
			fmt.Printf("Used @ %s but not defined: %s\n", v, k)
		}
	}
	for k, v := range defdStrings {
		if usedStrings[k] == "" {
			fail = true
			fmt.Printf("Defined @ %d but not used: %s\n", v, k)
		}
	}
	if fail {
		t.Fatal("failed")
	}
}

var built bool

func runSonar(t *testing.T, args ...string) string {
	if !built {
		err := exec.Command("sh", "-c", "cd ../.. ; cargo build").Run()
		if err != nil {
			t.Fatal(err)
		}
		built = true
	}
	cmdline := "../../target/debug/sonar " + strings.Join(args, " ")
	cmd := exec.Command("sh", "-c", cmdline)
	stdout, err := cmd.Output()
	if err != nil {
		t.Fatal(err)
	}
	return string(stdout)
}
