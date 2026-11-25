// Sample-scan reads and formats Sonar sample data from a directory tree.
//
// Usage:
//
//	sample-scan [options] directory from-date [to-date]
//
// Options:
//
//	-exe-re filename
//		Name a file of regular expression mappings for the exe field.
//
// The directory is the root of a Sonar data directory, ie, typically the directory name is a
// cluster name.  The dates are on the form yyyy-mm-dd and are inclusive, and will select
// directories.  Note that directories are named by UTC timestamps.
//
// The regular expression mapping format is currently:
//
// - Blank lines and lines whose first nonblank is '#' are ignored.
// - Remaining lines are interpreted in pairs: the first is the regex, the second is the
//   substitution
//
// The output is TBD but is currently a tab-separated line of field values.  Tabs in the field
// values themselves are replaced with single spaces.

// Misc optimizations
// - caching
// - regexes that are trivial (looks like Henrik has globs, really) are just prefix/substring/suffix matching
//   and can use that, though some have embedded patterns and may need the full thing

package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
)

var (
	exeRe = flag.String("exe-re", "", "`Filename` holding mappings for exe field")
)

func main() {
	FlagParse("sample-scan", "directory", "from-date", "?to-date")
	
}
