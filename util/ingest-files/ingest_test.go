// This is kind of dumb but ensures that things pretty much work.

package main

import (
	"reflect"
	"slices"
	"testing"
)

var files = []string{
	"testdata/sonar-output/a/c1-5.fox.csv",
	"testdata/sonar-output/a/c1-6.fox.csv",
	"testdata/sonar-output/a/c1-7.fox.csv",
	"testdata/sonar-output/a/c1-8.fox.csv",
	"testdata/sonar-output/a/c1-9.fox.csv",
	"testdata/sonar-output/a/sysinfo-c1-5.fox.json",
	"testdata/sonar-output/a/sysinfo-c1-6.fox.json",
	"testdata/sonar-output/a/sysinfo-c1-7.fox.json",
	"testdata/sonar-output/a/sysinfo-c1-8.fox.json",
	"testdata/sonar-output/a/sysinfo-c1-9.fox.json",
	"testdata/sonar-output/b/gpu-10.fox.csv",
	"testdata/sonar-output/b/gpu-11.fox.csv",
	"testdata/sonar-output/b/gpu-12.fox.csv",
	"testdata/sonar-output/b/gpu-13.fox.csv",
	"testdata/sonar-output/b/gpu-15.fox.csv",
	"testdata/sonar-output/b/slurm-sacct.csv",
	"testdata/sonar-output/b/sysinfo-gpu-10.fox.json",
	"testdata/sonar-output/b/sysinfo-gpu-11.fox.json",
	"testdata/sonar-output/b/sysinfo-gpu-12.fox.json",
	"testdata/sonar-output/b/sysinfo-gpu-13.fox.json",
	"testdata/sonar-output/b/sysinfo-gpu-15.fox.json",
}

func TestFilenames(t *testing.T) {
	mode = mFilenames
	collected = make(map[string]string)
	readFiles("testdata/sonar-output")
	names := make([]string, 0, len(collected))
	for k := range collected {
		names = append(names, k)
	}
	slices.Sort(names)
	if !reflect.DeepEqual(names, files) {
		t.Fatal("Not the expected files")
	}
}

var expected = []string{
	"testdata/sonar-output/a/c1-5.fox.csv c1-5.fox 2025-03-01T05:05:01+01:00 namd2 25781",
	"testdata/sonar-output/a/c1-6.fox.csv c1-6.fox 2025-03-01T05:05:01+01:00 mpihello 45042",
	"testdata/sonar-output/a/c1-7.fox.csv c1-7.fox 2025-03-01T05:00:01+01:00 java 56832",
	"testdata/sonar-output/a/c1-8.fox.csv c1-8.fox 2025-03-01T01:50:01+01:00 namd2 221627",
	"testdata/sonar-output/a/c1-9.fox.csv c1-9.fox 2025-03-01T02:05:01+01:00 analysisWGS.sh 1233393",
	"testdata/sonar-output/a/sysinfo-c1-5.fox.json c1-5.fox 2025-03-02T00:00:01+01:00 2x64 AMD EPYC 7702 64-Core Processor, 503 GiB 128 503",
	"testdata/sonar-output/a/sysinfo-c1-6.fox.json c1-6.fox 2025-03-02T00:00:01+01:00 2x64 AMD EPYC 7702 64-Core Processor, 503 GiB 128 503",
	"testdata/sonar-output/a/sysinfo-c1-7.fox.json c1-7.fox 2025-03-02T00:00:01+01:00 2x64 AMD EPYC 7702 64-Core Processor, 503 GiB 128 503",
	"testdata/sonar-output/a/sysinfo-c1-8.fox.json c1-8.fox 2025-03-02T00:00:01+01:00 2x64 AMD EPYC 7702 64-Core Processor, 503 GiB 128 503",
	"testdata/sonar-output/a/sysinfo-c1-9.fox.json c1-9.fox 2025-03-02T00:00:01+01:00 2x64 AMD EPYC 7702 64-Core Processor, 503 GiB 128 503",
	"testdata/sonar-output/b/gpu-10.fox.csv gpu-10.fox 2025-03-01T09:00:01+01:00 ",
	"testdata/sonar-output/b/gpu-11.fox.csv gpu-11.fox 2025-03-01T01:50:01+01:00 nvidia-smi 842",
	"testdata/sonar-output/b/gpu-12.fox.csv gpu-12.fox 2025-03-01T02:30:01+01:00 python 255",
	"testdata/sonar-output/b/gpu-13.fox.csv gpu-13.fox 2025-03-01T09:15:01+01:00 ",
	"testdata/sonar-output/b/gpu-15.fox.csv gpu-15.fox 2025-03-01T09:15:01+01:00 ",
	"testdata/sonar-output/b/slurm-sacct.csv Slurmjob ec-aaf 1355127_17 ec29",
	"testdata/sonar-output/b/sysinfo-gpu-10.fox.json gpu-10.fox 2025-03-02T00:00:01+01:00 2x48 (hyperthreaded) AMD EPYC 7642 48-Core Processor, 1007 GiB, 2x NVIDIA H100 PCIe @ 80GiB 192 1007",
	"testdata/sonar-output/b/sysinfo-gpu-11.fox.json gpu-11.fox 2025-03-02T00:00:01+01:00 2x48 (hyperthreaded) AMD EPYC 7642 48-Core Processor, 2003 GiB, 8x NVIDIA GeForce RTX 3090 @ 24GiB 192 2003",
	"testdata/sonar-output/b/sysinfo-gpu-12.fox.json gpu-12.fox 2025-03-02T00:00:01+01:00 2x48 (hyperthreaded) AMD EPYC 7642 48-Core Processor, 2003 GiB, 4x NVIDIA A40 @ 45GiB 192 2003",
	"testdata/sonar-output/b/sysinfo-gpu-13.fox.json gpu-13.fox 2025-03-02T00:00:01+01:00 2x48 (hyperthreaded) AMD EPYC 7642 48-Core Processor, 1007 GiB, 4x NVIDIA A100-PCIE-40GB @ 40GiB 192 1007",
	"testdata/sonar-output/b/sysinfo-gpu-15.fox.json gpu-15.fox 2025-03-02T00:00:01+01:00 2x64 (hyperthreaded) AMD EPYC 7H12 64-Core Processor, 1007 GiB, 4x NVIDIA L40S @ 45GiB 256 1007",
}

func TestParsing(t *testing.T) {
	mode = mContents
	collected = make(map[string]string)
	readFiles("testdata/sonar-output")
	output := make([]string, 0, len(collected))
	for k, v := range collected {
		output = append(output, k + " " + v)
	}
	slices.Sort(output)
	if !reflect.DeepEqual(output, expected) {
		t.Fatal("Not the expected output")
	}
}
