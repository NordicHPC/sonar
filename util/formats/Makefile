.PHONY: all test format generate clean

all:
	@echo "Pick an explicit target"

clean:
	go clean

generate:
	( cd newfmt ; go generate )

test:
	go test
	( cd newfmt ; go test )

format:
	go fmt
	( cd newfmt ; go fmt )
