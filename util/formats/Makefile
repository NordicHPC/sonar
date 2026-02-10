SHELL=/bin/bash

.PHONY: all test format generate

all:
	@echo "Pick an explicit target"

generate:
	@if [[ $$(command -v go) != "" ]]; then \
		( cd newfmt ; go generate ); \
	else \
		echo "Skipping formats generation - no go executable"; \
	fi

test:
	@if [[ $$(command -v go) != "" ]]; then \
		go test; \
		( cd newfmt ; go test ); \
	else \
		echo "Skipping formats test - no go executable"; \
	fi

format:
	@if [[ $$(command -v go) != "" ]]; then \
		go fmt; \
		( cd newfmt ; go fmt ); \
	else \
		echo "Skipping formats fmt - no go executable"; \
	fi
