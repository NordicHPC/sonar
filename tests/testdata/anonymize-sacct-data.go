// Anonymize-sacct-data replaces educloud user and account names in an sacct output file and the
// corresponding output file from Sonar.
//
// If -o is true then the input files are renamed to .bak files and the output files replace the
// input files.  Otherwise, the output is written to .new files.
package main

import (
	"flag"
	"fmt"
	"os"
	"regexp"
)

const (
	sacctOutput = "sacct_output.txt"
	sonarOutput = "sonar_sacct_output.txt"
)

var (
	overwrite = flag.Bool("o", false, "Overwrite inputs")
)

func main() {
	flag.Parse()
	sacctOutputBytes, err := os.ReadFile(sacctOutput)
	check(err)
	sacctTxt := string(sacctOutputBytes)
	sonarOutputBytes, err := os.ReadFile(sonarOutput)
	check(err)
	sonarTxt := string(sonarOutputBytes)

	sacctUserRe := regexp.MustCompile(`\|(ec-[a-zA-Z0-9]+)\|`)
	users := make(map[string]string)
	nameCount := 10000
	for _, k := range sacctUserRe.FindAllStringSubmatch(sacctTxt, -1) {
		name := k[1]
		if users[name] == "" {
			users[name] = fmt.Sprintf("uc-%d", nameCount)
			nameCount++
		}
	}

	sacctAcctRe := regexp.MustCompile(`\|(ec[0-9]+)\|`)
	accounts := make(map[string]string)
	acctCount := 10000
	for _, k := range sacctAcctRe.FindAllStringSubmatch(sacctTxt, -1) {
		name := k[1]
		if accounts[name] == "" {
			accounts[name] = fmt.Sprintf("ac%d", acctCount)
			acctCount++
		}
	}

	sacctTxt = sacctUserRe.ReplaceAllStringFunc(sacctTxt, func (s string) string {
		return `|` + users[s[1:len(s)-1]] + `|`
	})
	sacctTxt = sacctAcctRe.ReplaceAllStringFunc(sacctTxt, func (s string) string {
		return `|` + accounts[s[1:len(s)-1]] + `|`
	})

	sonarUserRe := regexp.MustCompile(`"user_name":"ec-[a-zA-Z0-9]+"`)
	sonarTxt = sonarUserRe.ReplaceAllStringFunc(sonarTxt, func (s string) string {
		return `"user_name":"` + users[s[13:len(s)-1]] + `"`
	})
	sonarAcctRe := regexp.MustCompile(`"account":"ec[0-9]+"`)
	sonarTxt = sonarAcctRe.ReplaceAllStringFunc(sonarTxt, func (s string) string {
		return `"account":"` + accounts[s[11:len(s)-1]] + `"`
	})

	if *overwrite {
		check(os.Rename(sacctOutput, sacctOutput + ".bak"))
		check(os.Rename(sonarOutput, sonarOutput + ".bak"))
		check(os.WriteFile(sacctOutput, []byte(sacctTxt), 0666))
		check(os.WriteFile(sonarOutput, []byte(sonarTxt), 0666))
	} else {
		check(os.WriteFile(sacctOutput + ".new", []byte(sacctTxt), 0666))
		check(os.WriteFile(sonarOutput + ".new", []byte(sonarTxt), 0666))
	}
}

func check(err error) {
	if err != nil {
		panic(err)
	}
}
