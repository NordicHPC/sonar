.PHONY: debug release test format
ARCH=$(shell uname -m)

ifeq (x86_64, $(ARCH))
	FEATURES=--features xpu,habana
endif

debug:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build $(FEATURES)

release:
	CARGO_TARGET_DIR=target/$(ARCH) cargo build --release $(FEATURES)

test: debug release
	cargo test
	( cd util ; $(MAKE) test )
	( cd tests; ./run_tests.sh )

format:
	cargo fmt
	( cd util ; $(MAKE) format )
