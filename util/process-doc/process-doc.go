// This reads input from stdin and extracts some documentation, and generates various output on
// stdout, depending on options.  A typical input file is ../formats/newfmt/types.go.  Typical
// output is markdown documentation, or field name definitions to be used by Rust code.  Try -h.
//
// The format is this:
//
// Input     ::= Preamble? (TypeDefn | Const | Junk)* Postamble?
// Preamble  ::= PreFlag Doc*
// PreFlag   ::= <line starting with "///+preamble" after blank stripping
// Postamble ::= PostFlag Doc*
// PostFlag  ::= <line starting with "///+postamble" after blank stripping
// TypeDefn  ::= Doc+ Blank* Type FieldDefn*
// Const     ::= <line of the form /const\s+Id\s+.+\s*=\s*ConstVal after blank stripping>
// ConstVal  ::= <string literal or unsigned literal>
// Doc       ::= <line starting with "///" after blank stripping>
// Blank     ::= <line that's empty after blank stripping>
// Type      ::= <contextually, line that starts with "type" after blank stripping>
// FieldDefn ::= Blank* Doc+ Blank* Field
// Field     ::= <contextually, line that starts with a capitalized identifier and has a json tag>
// Junk      ::= <any other line>
//
// Note that anything that is not a FieldDefn will interrupt the run of fields in a structured type.
//
// TODO:
// - obviously it would be fun to hyperlink automatically from uses of types to their definitions
// - consider emitting not string but enums or other type-safer things (see TODO below)
// - avoid stripping indentation in code blocks (see TODO below)
// - proper underline insertion in words like MyCPUAvg which should be MY_CPU_AVG (see TODO below)

package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"regexp"
	"strings"
)

var (
	makeDoc  = flag.Bool("doc", false, "Produce markdown documentation")
	makeRust = flag.Bool("tag", false, "Produce Rust constant JSON field tags")
	warnings = flag.Bool("w", false, "Print warnings")
)

func main() {
	flag.Parse()
	if *makeDoc == *makeRust {
		fmt.Fprintf(os.Stderr, "Must use -doc xor -tag.  Try -h.\n")
		os.Exit(2)
	}
	switch {
	case *makeDoc:
		fmt.Print("# Sonar JSON format output specification\n\n")
		fmt.Print("AUTOMATICALLY GENERATED BY `process-doc`.  DO NOT EDIT.\n")
		fmt.Print("Instead, edit `util/formats/newfmt/types.go`, then in `util/process-doc` run `make install`.\n\n")

	case *makeRust:
		fmt.Print("// AUTOMATICALLY GENERATED BY `process-doc`.  DO NOT EDIT.\n")
		fmt.Print("// Edit `util/formats/newfmt/types.go`, then in `util/process-doc` run `make install`.\n\n")
	}
	lines := make(chan any)
	go producer(lines)
	process(lines)
}

type DocLine struct {
	Lineno int
	Text   string
}

type TypeLine struct {
	Lineno int
	Name   string
}

type FieldLine struct {
	Lineno int
	Name   string
	Type   string
	Json   string
}

type ConstLine struct {
	Lineno int
	Name   string
	Value  string
}

type JunkLine struct {
	Lineno int
}

func process(lines <-chan any) {
	doc := make([]string, 0)
	var havePreamble bool
	var currLine any
	var todos int
LineConsumer:
	for {
		currLine = <-lines
	OuterSwitch:
		switch l := currLine.(type) {
		case nil:
			break LineConsumer
		case JunkLine:
			isPreamble := false
			if len(doc) > 0 && strings.HasPrefix(doc[0], "+preamble") {
				isPreamble = true
			}
			havePreamble, doc = maybePreamble(l.Lineno, havePreamble, doc)
			warnIf(len(doc) > 0 && !isPreamble, l.Lineno, "Junk following doc")
			doc = doc[0:0]
		case DocLine:
			doc = append(doc, l.Text)
			if strings.Index(l.Text, "TODO") >= 0 {
				todos++
			}
		case TypeLine:
			havePreamble, doc = maybePreamble(l.Lineno, havePreamble, doc)
			warnIf(len(doc) == 0, l.Lineno, "Type without doc")
			emitType(l, doc)
			doc = doc[0:0]
			currType := l.Name
		FieldConsumer:
			for {
				// Important that we reuse the currLine and doc variables
				currLine = <-lines
				switch l := currLine.(type) {
				case nil:
					break LineConsumer
				case JunkLine:
					warnIf(len(doc) > 0, l.Lineno, "Junk following doc")
					doc = doc[0:0]
					break FieldConsumer
				case DocLine:
					if strings.Index(l.Text, "TODO") >= 0 {
						todos++
					}
					doc = append(doc, l.Text)
				case TypeLine:
					goto OuterSwitch
				case ConstLine:
					goto OuterSwitch
				case FieldLine:
					warnIf(len(doc) == 0, l.Lineno, "Field without doc")
					emitField(l, currType, doc)
					doc = doc[0:0]
				default:
					panic("Should not happen")
				}
			}
		case FieldLine:
			havePreamble, doc = maybePreamble(l.Lineno, havePreamble, doc)
			warnIf(true, l.Lineno, "Field outside of typedecl context")
		case ConstLine:
			havePreamble, doc = maybePreamble(l.Lineno, havePreamble, doc)
			emitConst(l, doc)
			doc = doc[0:0]
		default:
			panic("Should not happen")
		}
	}
	if *makeDoc {
		if len(doc) > 0 && strings.HasPrefix(doc[0], "+postamble") {
			for _, d := range doc[1:] {
				fmt.Println(d)
			}
			fmt.Println()
		}
		if todos > 0 && *warnings {
			fmt.Fprintf(os.Stderr, "WARNING: At least %d TODOs in input\n", todos)
		}
	}
}

func maybePreamble(l int, havePreamble bool, doc []string) (bool, []string) {
	if len(doc) > 1 && !*makeRust {
		if strings.HasPrefix(doc[0], "+preamble") {
			warnIf(havePreamble, l, "Redundant preamble")
			for _, d := range doc[1:] {
				fmt.Println(d)
			}
			fmt.Println()
			return true, doc[0:0]
		}
	}
	return false, doc
}

func warnIf(cond bool, l int, msg string) {
	if cond && *warnings {
		fmt.Fprintf(os.Stderr, "%d: WARNING: %s\n", l, msg)
	}
}

var printedTypeHeading bool

func maybeTypeHeading() {
	if !printedTypeHeading {
		fmt.Print("## Data types\n\n")
		printedTypeHeading = true
	}
}

func emitType(l TypeLine, doc []string) {
	if *makeDoc {
		maybeTypeHeading()
		fmt.Printf("### Type: `%s`\n\n", l.Name)
		for _, d := range doc {
			fmt.Println(d)
		}
		fmt.Println()
	}
}

func emitField(l FieldLine, currType string, doc []string) {
	switch {
	case *makeDoc:
		fmt.Printf("#### **`%s`** %s\n\n", l.Json, l.Type)
		for _, d := range doc {
			fmt.Println(d)
		}
		fmt.Println()
	case *makeRust:
		// TODO: These are emitted as &str now.  But in a future universe, once all the old
		// formatting code is gone, or maybe even before, they could maybe be of a distinguished
		// type, to prevent literal strings from being used at all.  (It could be an enum wrapping a
		// &str, modulo problems with initialization, or maybe it would be an enum whose value
		// points into some table.)
		fmt.Printf("pub const %s: &str = \"%s\"; // %s\n", transformName(currType + l.Name), l.Json, l.Type)
	}
}

func emitConst(l ConstLine, doc []string) {
	if *makeRust {
		ty := "u64"
		if strings.HasPrefix(l.Value, "\"") {
			ty = "&str"
		}
		fmt.Printf("pub const %s: %s = %s;\n", transformName(l.Name), ty, l.Value)
	}
}

// Rust naming conventions: In a given name, the first capital letter X after a lower case
// letter is transformed to _X.
//
// TODO: _ should be inserted between the last two capitals of a run of capitals immediately
// followed by a lower case letter, so that 'CEClock' becomes '_CE_CLOCK_' no '_CECLOCK_'.

func transformName(n string) string {
	bs := []byte(n)
	name := ""
	for i := range bs {
		if i > 0 && isUpper(bs[i]) && !isUpper(bs[i-1]) {
			name += "_"
		}
		name += toUpper(bs[i])
	}
	return name
}

func isUpper(b uint8) bool {
	return b >= 'A' && b <= 'Z'
}

func isLower(b uint8) bool {
	return b >= 'a' && b <= 'z'
}

func toUpper(b uint8) string {
	if isLower(b) {
		return string(b - ('a' - 'A'))
	}
	return string(b)
}

var (
	docRe   = regexp.MustCompile(`^\s*///(.*)$`)
	blankRe = regexp.MustCompile(`^\s*$`)
	typeRe  = regexp.MustCompile(`^\s*type\s+([a-zA-Z0-9_]+)`)
	fieldRe = regexp.MustCompile(`^\s*([A-Z][a-zA-Z0-9_]*)\s+(.*)\s+` + "`" + `json:"(.*)"`)
	constRe = regexp.MustCompile(`^\s*const\s+([a-zA-Z0-9_]+)\s+.*=\s*([^\s]+)`)
)

func producer(lines chan<- any) {
	scanner := bufio.NewScanner(os.Stdin)
	var lineno int
	for scanner.Scan() {
		lineno++
		l := scanner.Text()
		if blankRe.MatchString(l) {
			continue
		}
		if m := docRe.FindStringSubmatch(l); m != nil {
			// TODO: We don't want to trim (too much) left space in code contexts b/c
			// of indentation.
			lines <- DocLine{Lineno: lineno, Text: strings.TrimSpace(m[1])}
			continue
		}
		if m := typeRe.FindStringSubmatch(l); m != nil {
			lines <- TypeLine{Lineno: lineno, Name: m[1]}
			continue
		}
		if m := fieldRe.FindStringSubmatch(l); m != nil {
			lines <- FieldLine{Lineno: lineno, Name: m[1], Type: strings.TrimSpace(m[2]), Json: m[3]}
			continue
		}
		if m := constRe.FindStringSubmatch(l); m != nil {
			lines <- ConstLine{Lineno: lineno, Name: m[1], Value: m[2]}
			continue
		}
		lines <- JunkLine{lineno}
	}
	close(lines)
}
