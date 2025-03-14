// Sample code: Ingest old-format files.
//
// The trailing arguments are names of files or directories.  If they are files they must match
// 'slurm-sacct.csv' for old-style `sonar slurm` data, '*.csv' for old-style `sonar ps` data, or
// `sysinfo-*.json` for old-style `sonar sysinfo` data.  Otherwise they are going to be treated as
// directories that we traverse.  Files we don't know what to do with are ignored.
//
// Also see ingest_test.go.

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

// Testing stuff
const (
	mPrint = iota
	mFilenames
	mContents
)

var (
	mode = mPrint
	collected map[string]string
)

func main() {
	// TODO: Usage message to indicate trailing args
	flag.Parse()
	readFiles(flag.Args()...)
}

func readFiles(fileNames ...string) {
	for _, candidate := range fileNames {
		if tryMatch(candidate) {
			continue
		}
		fs.WalkDir(os.DirFS(candidate), ".", func(fpath string, d fs.DirEntry, err error) error {
			if err != nil {
				log.Fatal(err)
			}
			if !d.IsDir() {
				tryMatch(path.Join(candidate, fpath))
			}
			return nil
		})
	}
}

var (
	// sacctFile must be tested before sampleFile: the latter is a generalization of the former.
	sacctFile   = regexp.MustCompile(`^(?:.*/)?slurm-sacct\.csv$`)
	sampleFile  = regexp.MustCompile(`^(?:.*/)?([^/]*)\.csv$`)
	sysinfoFile = regexp.MustCompile(`^(?:.*/)?sysinfo-([^/]*)\.json$`)
)

func tryMatch(candidate string) bool {
	if m := sacctFile.FindStringSubmatch(candidate); m != nil {
		consumeOldSacctFile(candidate)
		return true
	}
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
	oldfmt.ConsumeCSVSamples(f, func (info *oldfmt.SampleEnvelope) {
		consumeOldSample(fn, info)
	})
}

func consumeOldSysinfoFile(fn, hostname string) {
	f, err := os.Open(fn)
	if err != nil {
		log.Fatal(err)
	}
	defer f.Close()
	oldfmt.ConsumeJSONSysinfo(f, func (info *oldfmt.SysinfoEnvelope) {
		consumeOldSysinfo(fn, info)
	})
}

func consumeOldSacctFile(fn string) {
	f, err := os.Open(fn)
	if err != nil {
		log.Fatal(err)
	}
	defer f.Close()
	oldfmt.ConsumeCSVSlurmJobs(f, func (info any) {
		consumeOldSlurmJob(fn, info)
	})
}

// These are just example consumers.

func consumeOldSysinfo(source string, info *oldfmt.SysinfoEnvelope) {
	switch mode {
	case mFilenames:
		collected[source] = ""
	case mContents:
		collected[source] = fmt.Sprint(
			info.Hostname, " ", info.Timestamp, " ", info.Description, " ", info.CpuCores, " ", info.MemGB)
	case mPrint:
		fmt.Printf("Sysinfo for %s %s from %s\n", info.Hostname, info.Timestamp, source)
		fmt.Printf("  Desc %s\n  CpuCores %d\n  MemGB %d\n",
			info.Description, info.CpuCores, info.MemGB)
	}
}

func consumeOldSample(source string, info *oldfmt.SampleEnvelope) {
	switch mode {
	case mFilenames:
		collected[source] = ""
	case mContents:
		var x string
		for _, s := range info.Samples {
			if s.CpuTimeSec > 100 {
				x = fmt.Sprint(s.Cmd, " ", s.CpuTimeSec)
			}
		}
		collected[source] = fmt.Sprint(info.Hostname, " ", info.Timestamp, " ", x)
	case mPrint:
		fmt.Printf("Sample for %s %s from %s\n", info.Hostname, info.Timestamp, source)
		for _, s := range info.Samples {
			if s.CpuTimeSec > 100 {
				fmt.Printf("  `%s` %d\n", s.Cmd, s.CpuTimeSec)
			}
		}
	}
}

func consumeOldSlurmJob(source string, info any) {
	switch mode {
	case mFilenames:
		collected[source] = ""
	case mContents:
		switch i := info.(type) {
		case *oldfmt.SlurmEnvelope:
			var x string
			if len(i.Jobs) > 0 {
				x = fmt.Sprint(i.Jobs[0].User, " ", i.Jobs[0].JobID, " ", i.Jobs[0].Account)
			}
			collected[source] = fmt.Sprint("Slurmjob ", x)
		case *oldfmt.SlurmErrorEnvelope:
			panic("Should not happen")
		}
	case mPrint:
		switch i := info.(type) {
		case *oldfmt.SlurmEnvelope:
			fmt.Printf("Slurmjob for from %s\n", source)
			if len(i.Jobs) > 0 {
				fmt.Printf("  User %s\n  JobID %s\n  Account %s\n",
					i.Jobs[0].User, i.Jobs[0].JobID, i.Jobs[0].Account)
			}
		case *oldfmt.SlurmErrorEnvelope:
			panic("Should not happen")
		}
	}
}
