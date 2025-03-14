// Ingest old-format files.  The trailing arguments are names of files or directories.  If they are
// files they must match '*.csv' for old-style `sonar ps` data or `sysinfo-*.json` for old-style
// `sonar sysinfo` data.  Otherwise they are going to be treated as directories that we traverse.

package main

import (
	"flag"
	"fmt"
	"io/fs"
	"log"
	"os"
	"path"
	"regexp"

	"github.com/NordicHPC/sonar/util/formats/oldfmt"
)

var (
	verbose = flag.Bool("v", false, "Verbose")
)

func main() {
	// TODO: Usage message to indicate trailing args
	flag.Parse()
	for _, candidate := range flag.Args() {
		if tryMatch(candidate) {
			continue
		}
		fs.WalkDir(os.DirFS(candidate), ".", func(fpath string, _ fs.DirEntry, err error) error {
			if err != nil {
				log.Fatal(err)
			}
			tryMatch(path.Join(candidate, fpath))
			return nil
		})
	}
}

var (
	sampleFile  = regexp.MustCompile(`^(?:.*/)?([^/]*)\.csv$`)
	sysinfoFile = regexp.MustCompile(`^(?:.*/)?sysinfo-([^/]*)\.json$`)
)

func tryMatch(candidate string) bool {
	if m := sampleFile.FindStringSubmatch(candidate); m != nil {
		consumeOldSampleFile(candidate, m[1])
		return true
	}
	if m := sysinfoFile.FindStringSubmatch(candidate); m != nil {
		consumeOldSysinfoFile(candidate, m[1])
		return true
	}
	return false
}

func consumeOldSampleFile(fn, hostname string) {
	f, err := os.Open(fn)
	if err != nil {
		log.Fatal(err)
	}
	defer f.Close()
	oldfmt.ConsumeCSVSamples(f, consumeOldSample)
}

func consumeOldSysinfoFile(fn, hostname string) {
	f, err := os.Open(fn)
	if err != nil {
		log.Fatal(err)
	}
	defer f.Close()
	oldfmt.ConsumeJSONSysinfo(f, consumeOldSysinfo)
}

// These are just example consumers.

func consumeOldSysinfo(info *oldfmt.SysinfoEnvelope) {
	fmt.Printf("Sysinfo for %s %s\n", info.Hostname, info.Timestamp)
	fmt.Printf("  Desc %s\n  CpuCores %d\n  MemGB %d\n", info.Description, info.CpuCores, info.MemGB)
}

func consumeOldSample(info *oldfmt.SampleEnvelope) {
	fmt.Printf("Sample for %s %s\n", info.Hostname, info.Timestamp)
	for _, s := range info.Samples {
		if s.CpuTimeSec > 100 {
			fmt.Printf("  `%s` %d\n", s.Cmd, s.CpuTimeSec)
		}
	}
}
