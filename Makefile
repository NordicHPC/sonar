.PHONY: debug release build test format generate default install
ARCH=$(shell uname -m)

# Normally the default target is not quite what you want but it's OK
# for casual use and to emulate a standard build process.

default: target/release/sonar

target/release/sonar: src/*.rs src/*/*.rs gpuapi/$(ARCH)/*.a
	cargo build --release

src/json_tags.rs: util/formats/newfmt/types.go
	$(MAKE) generate

# This is here just to conform to a standard build process.

install: target/release/sonar
	@echo ""
	@echo "Sonar installation must be performed manually, see doc/MANUAL.md"
	@echo ""

# This is where the real action is, see doc/HOWTO-DEVELOP.md.

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
	( cd build-dist ; $(MAKE) generate )
	( cd util ; $(MAKE) generate )
	( cd tests ; $(MAKE) generate )

# https://github.com/lars-t-hansen/gotags, you can also use etags here for less interesting info
RSSRC=$(shell find src -name '*.rs')
GOSRC=$(shell find util -name '*.go')
CSRC=$(shell find gpuapi -name '*.c' -o -name '*.h')
SRC=$(RSSRC) $(GOSRC) $(CSRC) build.rs
TAGS: $(SRC)
	@gotags $(SRC)
	@echo "TAGS rebuilt"
