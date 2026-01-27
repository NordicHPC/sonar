.PHONY: all test format

all:
	@echo "Select a specific target"

test:
	( cd formats ; $(MAKE) test )
	( cd kafka-proxy ; $(MAKE) test )

format:
	( cd formats ; $(MAKE) format )
	( cd kafka-proxy ; $(MAKE) format )

