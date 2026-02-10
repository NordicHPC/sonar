.PHONY: debug release build test format generate
ARCH=$(shell uname -m)

debug:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build

release:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build --release

build: generate format release

# Build everything then run all test suites
test: debug release
	cargo test
	( cd util ; $(MAKE) test )
	( cd tests; $(MAKE) test )

# Reformat all sources
format:
	cargo fmt
	( cd util ; $(MAKE) format )
	( cd tests ; $(MAKE) format )
	( cd gpuapi ; $(MAKE) format )

# (Re)generate all files that are generated
generate:
	( cd util ; $(MAKE) generate )
	( cd tests ; $(MAKE) generate )
