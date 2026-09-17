package main

import (
	"encoding/json"
	"github.com/NordicHPC/sonar/util/formats/newfmt"
	"io"
	"os"
	"testing"
)

func TestSniff(t *testing.T) {
	f, _ := os.Open("testdata/p1/nix.json")
	defer f.Close()
	ty, err := SniffType(f)
	if err != nil {
		t.Fatal(err)
	}
	if ty != newfmt.DataTagSysinfo {
		t.Fatal("Bad tag: " + ty)
	}
	dec := json.NewDecoder(f)
	var m newfmt.SysinfoEnvelope
	err = dec.Decode(&m)
	if err != nil {
		t.Fatal(err)
	}
	if m.Data.Attributes.Time != "2026-09-09T09:09:09Z" {
		t.Fatal("Bad timestamp: " + m.Data.Attributes.Time)
	}
}

func TestEof(t *testing.T) {
	f, _ := os.Open("/dev/null")
	defer f.Close()
	_, err := SniffType(f)
	if err != io.EOF {
		t.Fatal(err)
	}
}
