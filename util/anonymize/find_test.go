package main

import (
	"slices"
	"strings"
	"testing"
)

func TestGlob(t *testing.T) {
	files, err := GlobDir(".", "*.json")
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Equal(files, []string{
		"testdata/p1/nix.json",
		"testdata/p1/p1.1/zupp.json",
		"testdata/p2/p2.1/abra.json",
		"testdata/p2/p2.1/cadabra.json",
		"testdata/p2/zwapp.json"}) {
		t.Fatalf("File lists: %v", files)
	}
}

func TestFind(t *testing.T) {
	// list1.txt points to list2.txt in that directory, so this tests nested inclusion too.
	files, err := FindFiles([]string{"@testdata/list1.txt", "testdata/p2"}, "*.json")
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Equal(files, []string{
		"testdata/p1/p1.1/zupp.json",
		"testdata/p2/p2.1/abra.json",
		"testdata/p2/p2.1/cadabra.json",
		"testdata/p2/zwapp.json"}) {
		t.Fatalf("File lists: %v", files)
	}

	_, err = FindFiles([]string{"@testdata/list3.txt"}, "*.json")
	if err == nil || !strings.Contains(err.Error(), " nested ") {
		t.Fatal("Unbounded recursion not detected")
	}
}
