.PHONY: all test

all:
	@echo "Select a specific target"

test:
	cargo test
	( cd util ; $(MAKE) test )
	( cd tests; ./run_tests.sh )
