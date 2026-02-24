.PHONY: all test format generate
SUBDIRS=formats ingest-kafka kafka-proxy process-doc

all:
	@echo "Select a specific target"

generate:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) generate ); done

test:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) test ); done

format:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) format ); done

