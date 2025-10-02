.PHONY: debug release test format
ARCH=$(shell uname -m)

debug:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build

release:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build --release

test: debug release
	cargo test
	( cd util ; $(MAKE) test )
	( cd tests; ./run_tests.sh )

format:
	cargo fmt
	( cd util ; $(MAKE) format )
	( cd tests ; $(MAKE) format )
	( cd gpuapi ; $(MAKE) format )
