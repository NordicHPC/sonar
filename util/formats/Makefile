.PHONY: all test format

all:
	@echo "Pick an explicit target"

test:
	@if [[ $$(command -v go) != "" ]]; then \
		go test; \
	else \
		echo "Skipping formats test - no go executable"; \
	fi

format:
	@if [[ $$(command -v go) != "" ]]; then \
		go fmt; \
	else \
		echo "Skipping formats fmt - no go executable"; \
	fi
