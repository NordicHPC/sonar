// Usage: version.go toml-file spec-file
//
// Given a .toml file and a .spec file, grab the version number from the toml and insert it where it
// is needed in the .spec, which is in Version: and in the Source0: directive.  Other lines are left
// unchanged.  The .spec is replaced with the rewritten version if everything's successful.
package main

import (
	"bufio"
	"fmt"
	"os"
	"path"
	"regexp"
	"strings"
)

func main() {
	if len(os.Args) != 3 || len(os.Args) > 1 && os.Args[1] == "-h" {
		fmt.Fprintf(os.Stderr, "Usage: %s toml-file spec-file\n", os.Args[0])
		os.Exit(2)
	}
	tomlName := os.Args[1]
	specName := os.Args[2]

	version := readToml(tomlName)

	temp, err := os.CreateTemp(path.Dir(specName), "version")
	if err != nil {
		fmt.Fprintf(os.Stderr, "Can't create temp file\n")
		os.Exit(1)
	}

	rewriteSpec(specName, temp, version)

	tempName := temp.Name()
	temp.Close()
	err = os.Rename(tempName, specName)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Can't rename temp to spec-file %s %v\n", specName, err)
		_ = os.Remove(tempName)
		os.Exit(1)
	}
}

func readToml(tomlName string) string {
	tomlVersionRe := regexp.MustCompile(`^version\s*=\s*"([^"]*)"`)
	toml, err := os.Open(tomlName)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Can't open toml-file %s, try -h\n", tomlName)
		os.Exit(1)
	}
	defer toml.Close()

	s := bufio.NewScanner(toml)
	var version string
	for s.Scan() {
		if m := tomlVersionRe.FindStringSubmatch(s.Text()); m != nil {
			version = m[1]
			break
		}
	}
	if version == "" {
		fmt.Fprintf(os.Stderr, "No version setting found in toml-file %s\n", tomlName)
		os.Exit(1)
	}
	return version
}

func rewriteSpec(specName string, out *os.File, version string) {
	versionRe := regexp.MustCompile(`^(Version:\s*)[^\s]*(.*)$`)
	sourceRe := regexp.MustCompile(`^(Source0:.*tags/v).*(\.tar\.gz.*)$`)
	prerelease := strings.Index(version, "-") != -1
	if prerelease {
		fmt.Fprintf(os.Stderr, "Can't encode prerelease %s, skipping translation\n", version)
	}
	spec, err := os.Open(specName)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Can't open spec-file %s, try -h\n", specName)
		os.Exit(1)
	}
	defer spec.Close()

	s := bufio.NewScanner(spec)
	for s.Scan() {
		l := s.Text()
		if prerelease {
			fmt.Fprintln(out, l)
		} else if m := versionRe.FindStringSubmatch(l); m != nil {
			fmt.Fprintln(out, m[1]+version+m[2])
		} else if m := sourceRe.FindStringSubmatch(l); m != nil {
			fmt.Fprintln(out, m[1]+version+m[2])
		} else {
			fmt.Fprintln(out, l)
		}
	}
}
