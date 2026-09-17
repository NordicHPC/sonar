// Anonymize will take a list of Sonar JSON-encoded data files and rewrite sensitive fields by
// systematically replacing them with random strings.  This allows anonymized Sonar data to be made
// available publicly, eg for testing.
//
// Usage:
//
//	anonymize [options] item ...
//
// Each item can be a literal filename.json, a filename list file on the form @filename-list, or a
// directory name.  If a filename list each filename is read from the list.  If a directory, the
// directory and its subdirectories are scanned for files of the form *.json.  Either way we
// end up with a list [filename.json, ...]
//
// Each filename.json in the list is copied into filename.json.bak (in the same directory) and then
// the anonymized file is placed in its stead if the field rewriting succeeded.  This replacement
// happens only if the file type has replaceable fields and no errors occur.
//
// If a file cannot be processed because its records cannot be parsed as a known type then an error
// message is printed on stderr, processing continues with the next filename in the list, and
// (unless -x is passed) the eventual exit from anonymize is an error exit.
//
// Each file can contain only one record type, which is sniffed by attempting to parse it when the
// file is opened.  The form of the filename is immaterial.
//
// Options:
//
//	-n               Print the resolved list of file names, but do not execute any renaming
//	-x               Do not error out for unknown content, leave those files alone
//	-feeling-lucky   Do not create .bak files
//	-d               Dump the mappings on stderr after a successful run
//
// The fields that are rewritten are hard-coded below.  Data type names and field names are relative
// to the definitions in sonar/util/formats/newfmt.
//
// Below SysinfoEnvelope:
//
//	none
//
// Below SampleEnvelope:
//
//	.Data.Attributes.Jobs.[].User
//
// Below JobsEnvelope:
//
//	.Data.Attributes.SlurmJobs.[].JobName
//	ditto UserName
//	ditto Account
//	ditto Reservation, if not blank
//
// Below ClusterEnvelope:
//
//	none
//
// Randomization is stable within a single run of the anonymizer for a given field across all
// records in all files.  The string "frank", in Sample's User field, will map to some random string
// "zxv123", and since UserName is also a user field, a value of "frank" in that field will map to
// "zxv123" also.  But Account and Reservation have their own mapping spaces.
package main

import (
	"cmp"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"github.com/NordicHPC/sonar/util/formats/newfmt"
	"io"
	"log"
	"os"
	"path"
	"slices"
)

var (
	dryRun    = flag.Bool("n", false, "Dry run: resolve items and print the list but do not rewrite anything")
	keepGoing = flag.Bool("x", false, "Keep going: do not error out for unhandled files")
	lucky     = flag.Bool("feeling-lucky", false, "Do not create .bak files")
	debug     = flag.Bool("d", false, "Debug: dump mappings after a successful run")
)

var notTranslated = errors.New("Not translated")

func main() {
	flag.Parse()
	filenames, err := FindFiles(flag.Args(), "*.json")
	if err != nil {
		log.Fatal(err)
	}
	if *dryRun {
		for _, fn := range filenames {
			fmt.Println(fn)
		}
		return
	}
	temps := make([]string, len(filenames))
	var failures int
	for i, filename := range filenames {
		infile, err := os.Open(filename)
		if err != nil {
			log.Print(err)
			failures++
			continue
		}
		tempfile, err := os.CreateTemp(path.Dir(filename), "anonymize*")
		if err != nil {
			log.Print(err)
			failures++
			infile.Close()
			continue
		}
		tempname := tempfile.Name()
		err = rewrite(infile, tempfile)
		tempfile.Close()
		infile.Close()
		if err != nil {
			if err != notTranslated {
				log.Print(err)
				failures++
			}
			os.Remove(tempname)
		} else {
			temps[i] = tempname
		}
	}
	if failures > 0 && !*keepGoing {
		for _, tempname := range temps {
			if tempname != "" {
				os.Remove(tempname)
			}
		}
		os.Exit(1)
	}
	for i := range filenames {
		if temps[i] != "" {
			if !*lucky {
				if err := os.Rename(filenames[i], filenames[i]+".bak"); err != nil {
					log.Fatal(err)
				}
			}
			if err := os.Rename(temps[i], filenames[i]); err != nil {
				log.Fatal(err)
			}
		}
	}
	if *debug {
		dumpMappings()
	}
}

var (
	jobnames     = make(map[string]string)
	users        = make(map[string]string)
	accounts     = make(map[string]string)
	reservations = make(map[string]string)
)

func rewrite(infile, outfile *os.File) error {
	ty, err := SniffType(infile)
	if err == io.EOF {
		// Empty file, we still do the backup + copy but this is trivially true.
		return nil
	}
	if err != nil {
		return err
	}
	dec := json.NewDecoder(infile)
	enc := json.NewEncoder(outfile)
	switch ty {
	case newfmt.DataTagSample:
		return mapJSON(dec, enc, func(m *newfmt.SampleEnvelope) {
			jobs := m.Data.Attributes.Jobs
			for i := range jobs {
				jobs[i].User = newfmt.NonemptyString(randstring(users, string(jobs[i].User), "user"))
			}
		})
	case newfmt.DataTagSysinfo:
		return notTranslated
	case newfmt.DataTagJobs:
		return mapJSON(dec, enc, func(m *newfmt.JobsEnvelope) {
			jobs := m.Data.Attributes.SlurmJobs
			for i := range jobs {
				jobs[i].JobName = randstring(jobnames, jobs[i].JobName, "job")
				jobs[i].UserName = randstring(users, jobs[i].UserName, "user")
				jobs[i].Account = randstring(accounts, jobs[i].Account, "acct")
				jobs[i].Reservation = randstring(reservations, jobs[i].Reservation, "resv")
			}
		})
	case newfmt.DataTagCluster:
		return notTranslated
	default:
		return err
	}
}

func mapJSON[T any](dec *json.Decoder, enc *json.Encoder, transform func(m *T)) error {
	for dec.More() {
		var m T
		err := dec.Decode(&m)
		if err != nil {
			return err
		}
		transform(&m)
		err = enc.Encode(&m)
		if err != nil {
			return err
		}
	}
	return nil
}

type kv struct {
	key, value string
}

func dumpMappings() {
	dumpKV("Jobnames", jobnames)
	dumpKV("Users", users)
	dumpKV("Accounts", accounts)
	dumpKV("Reservations", reservations)
}

func dumpKV(banner string, mapping map[string]string) {
	fmt.Fprintln(os.Stderr, banner)
	var xs []kv
	for k, v := range mapping {
		xs = append(xs, kv{k, v})
	}
	slices.SortFunc(xs, func(x, y kv) int {
		return cmp.Compare(x.key, y.key)
	})
	for _, kv := range xs {
		fmt.Fprintf(os.Stderr, "  %s\t%s\n", kv.key, kv.value)
	}
	fmt.Fprintln(os.Stderr, "")
}

var counter = 1000000

func randstring(domain map[string]string, s, prefix string) string {
	if s == "" {
		return s
	}
	if probe := domain[s]; probe != "" {
		return probe
	}
	counter++
	n := prefix + "_" + fmt.Sprint(counter)[1:]
	domain[s] = n
	return n
}
