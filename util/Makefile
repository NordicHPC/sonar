.PHONY: all test format generate

all:
	@echo "Select a specific target"

generate:
	( cd formats ; $(MAKE) generate )
	( cd process-doc ; $(MAKE) generate )
	( cd kafka-proxy ; $(MAKE) generate )

test:
	( cd formats ; $(MAKE) test )
	( cd kafka-proxy ; $(MAKE) test )

format:
	( cd formats ; $(MAKE) format )
	( cd kafka-proxy ; $(MAKE) format )

