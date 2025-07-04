.PHONY: all test format

all:
	@echo "Select a specific target"

test:
	( cd formats ; $(MAKE) test )
	( cd ingest-files ; $(MAKE) test )

format:
	( cd formats ; $(MAKE) format )
	( cd ingest-files ; $(MAKE) format )

