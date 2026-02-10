.PHONY: all release test format generate

all:
	@echo "Select a specific target"

# By default build a statically linked executable with CGO_ENABLED=0, easy to copy into containers
# etc.
#
# Possibly we also want -tags netgo but so far it's not been necessary.
#
# Use -a to force a rebuild of all dependencies because casual use of 'go build' during dev will
# create a libc dependency and it may not be a direct dependency.

generate:
	go generate

release: kprox.go version.go ../../Cargo.toml Makefile generate
	CGO_ENABLED=0 go build -a

test:
	go test
	$(MAKE) release

format:
	go fmt

