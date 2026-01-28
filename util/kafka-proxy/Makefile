.PHONY: all build test format

all:
	@echo "Select a specific target"

build: kprox.go version.go ../../Cargo.toml
	go generate
	go build

test: build
	go test

format:
	go fmt

