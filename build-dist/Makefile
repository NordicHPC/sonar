.PHONY: generate

generate: *.spec

*.spec: ../Cargo.toml
	for f in *.spec; do go run version.go ../Cargo.toml $$f; done
