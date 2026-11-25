// SPDX-License-Identifier: MIT

// From https://github.com/lars-t-hansen/util/go-utils, since modified.

package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"strings"
)

// Common superstructure for flag parsing for command line utilities.  Augments the usage message
// and checks the number of rest arguments.
//
// Trailing specs any trailing args.  Basically if we have ["a" "b" "..."] then we require one arg
// and the bs are optional and there can be many.  If we have ["a" "b"] then we require exactly 2.
// There may not be a "..." by itself.
func FlagParse(command string, trailing ...string) []string {
	var required int
	var optional bool
	if len(trailing) > 0 {
		if trailing[len(trailing)-1] == "..." {
			if len(trailing) == 1 {
				log.Fatal("Bad argument spec")
			}
			optional = true
			required = len(trailing) - 2
		} else {
			required = len(trailing)
		}
	}
	var additional string
	if len(trailing) > 0 {
		additional = " " + strings.Join(trailing, " ")
	}
	flag.Usage = func() {
		fmt.Fprintf(flag.CommandLine.Output(), "Usage of %s:\n", os.Args[0])
		fmt.Fprintf(flag.CommandLine.Output(), "%s [options]%s\nOptions:\n", command, additional)
		flag.PrintDefaults()
	}
	flag.Parse()
	rest := flag.Args()
	if optional && len(rest) < required || !optional && len(rest) != required {
		FlagFail("Missing rest argument(s)")
	}
	return rest
}

func FlagFail(msg string) {
	fmt.Fprintf(flag.CommandLine.Output(), "Argument error: %s\n\n", msg)
	flag.Usage()
	os.Exit(2)
}
