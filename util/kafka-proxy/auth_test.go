package main

import (
	"os"
	"testing"
)

func TestAuth(t *testing.T) {
	f, err := os.Open("testdata/auth_test1.txt")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	oracle, err := NewAuthenticator(f)
	if err != nil {
		t.Fatal(err)
	}
	if !oracle.Authenticate("grunge", "dirge") {
		t.Fatalf("Failed #1")
	}
	if oracle.Authenticate("grunge", "blapp") {
		t.Fatalf("Failed #2")
	}
	if !oracle.Authenticate("fuzz", "fizz") {
		t.Fatalf("Failed #3")
	}
	if oracle.Authenticate("blum", "fuzz") {
		t.Fatalf("Failed #4")
	}
}
