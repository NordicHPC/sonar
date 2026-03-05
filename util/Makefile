.PHONY: all test format generate clean
SUBDIRS=formats ingest-kafka kafka-proxy process-doc

all:
	@echo "Select a specific target"

clean:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) clean ); done

generate:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) generate ); done

test:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) test ); done

format:
	@for i in $(SUBDIRS); do ( cd $$i ; $(MAKE) format ); done

