// Subroutine for exclude-system-jobs.sh.
//
// Stdin is a series of JSON lines on the form [username,flagval,...] where flagval is null or true
// and the user name is double-quoted.  We're checking that if the uid of the user is < 1000 then at
// least one of the flagvals is true.  If a condition does not hold for a line then print the line
// on stderr and exit(1).

package main

import (
	"bufio"
	"fmt"
	"os"
	"os/user"
	"strings"
)

func main() {
	scanner := bufio.NewScanner(os.Stdin)
	var foundContained int
	for scanner.Scan() {
		xs := strings.Split(strings.Trim(scanner.Text(), "[]"), ",")
		name := strings.Trim(xs[0], "\"")
		u, err := user.Lookup(name)
		if err != nil {
			fmt.Printf("Warning: unknown user %s\n", name)
			continue
		}
		var uid uint
		fmt.Sscanf(u.Uid, "%d", &uid)
		if uid >= 1000 {
			continue
		}
		contained := false
		for _, f := range xs[1:] {
			contained = contained || f == "true"
		}
		if !contained {
			fmt.Fprintln(os.Stderr, "System job without any container processes")
			fmt.Fprintln(os.Stderr, scanner.Text())
			os.Exit(1)
		}
		foundContained++
	}
	fmt.Printf(" System user jobs with container process: %d\n", foundContained)
}
