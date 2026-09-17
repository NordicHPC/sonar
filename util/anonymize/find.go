package main

import (
	"bufio"
	"fmt"
	"io/fs"
	"os"
	"path"
	"slices"
	"strings"
)

const (
	maxDepth = 8
)

// FindFiles resolves indirections and directory names in the args, returning a list of plain file
// names where the files were known to exist at the time they were included in the list.  Files in
// directories are enumerated with the given glob, which should not have a path component.
// Indirection files can be nested.
//
// Blank lines in @indirection-file files are skipped.  Lines are assumed to name files relative to
// the indirection-file itself.
//
// Returned filenames are cleaned, sorted, and duplicates are removed.
func FindFiles(args []string, glob string) ([]string, error) {
	filenames, err := findFilesRaw(args, glob, 0)
	if err != nil {
		return nil, err
	}
	for i := range filenames {
		filenames[i] = path.Clean(filenames[i])
	}
	slices.Sort(filenames)
	return slices.Compact(filenames), nil
}

func findFilesRaw(args []string, glob string, depth int) ([]string, error) {
	filenames := make([]string, 0, len(args))
	for _, f := range args {
		files, err := findNamed(f, glob, depth)
		if err != nil {
			return nil, err
		}
		filenames = append(filenames, files...)
	}
	return filenames, nil
}

func findNamed(f, glob string, depth int) ([]string, error) {
	if f == "" {
		return nil, fmt.Errorf("Empty file name")
	}
	if f[0] == '@' {
		indirname := f[1:]
		if indirname == "" {
			return nil, fmt.Errorf("Empty indirection name '@'")
		}
		if depth == maxDepth {
			return nil, fmt.Errorf("Indirection files nested too deeply")
		}
		indir, err := os.Open(indirname)
		if err != nil {
			return nil, err
		}
		defer indir.Close()
		dirname := path.Dir(indirname)
		names := make([]string, 0)
		scanner := bufio.NewScanner(indir)
		for scanner.Scan() {
			t := strings.TrimSpace(scanner.Text())
			if t == "" {
				continue
			}
			var fn string
			if t != "" && t[0] == '@' {
				fn = "@" + path.Join(dirname, t[1:])
			} else {
				fn = path.Join(dirname, t)
			}
			names = append(names, fn)
		}
		return findFilesRaw(names, glob, depth+1)
	}

	info, err := os.Stat(f)
	if err == nil {
		if info.Mode()&fs.ModeType == 0 {
			return []string{f}, nil
		}
		if info.Mode()&fs.ModeDir != 0 {
			return GlobDir(f, glob)
		}
	}
	return nil, fmt.Errorf("The item %s is neither file nor directory", f)
}

// GlobDir walks the directory tree at dir and matches all plain files against glob, returning all
// matching file names.  Glob must have no pathname component.
func GlobDir(dir, glob string) ([]string, error) {
	files := make([]string, 0)
	err := fs.WalkDir(os.DirFS(dir), ".", func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		if info.Mode()&fs.ModeType == 0 {
			matched, err := path.Match(glob, d.Name())
			if err != nil {
				panic("Bad glob: " + glob)
			}
			if matched {
				files = append(files, path.Join(dir, p))
			}
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	return files, nil
}
