.PHONY: all test

all:
	@echo "Pick an explicit target"

test:
	@if [[ $$(command -v go) != "" ]]; then \
		go test; \
	else \
		echo "Skipping formats test - no go executable"; \
	fi
