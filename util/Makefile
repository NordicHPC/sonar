.PHONY: all test format generate licenses
SUBDIRS=anonymize formats ingest-kafka kafka-proxy process-doc

all:
	@echo "Select a specific target"

generate:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) generate ); done

test:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) test ); done

format:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) format ); done

# For this, you must first install go-licenses: https://github.com/google/go-licenses
# It will regenerate go-licenses.txt.
licenses:
	export SUBDIRS="$(SUBDIRS)" ; ./go-licenses.bash
	./rust-licenses.bash
