.PHONY: debug release test
ARCH=$(shell uname -m)

debug:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build

release:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build --release

test: debug release
	cargo test
	( cd util ; $(MAKE) test )
	( cd tests; ./run_tests.sh )
