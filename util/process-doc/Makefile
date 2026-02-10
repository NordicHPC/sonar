.PHONY: install generate

INPUT=../formats/newfmt/types.go

# NOTE!! When adding a target here, be sure to update ../../tests/go-doc-generate.sh

all: NEW-FORMAT.md json_tags.rs types.spec.yaml

generate: all

version.go: ../../Cargo.toml version.bash
	go generate

NEW-FORMAT.md: $(INPUT) process-doc
	./process-doc -w -doc < $(INPUT) > NEW-FORMAT.md

json_tags.rs: $(INPUT) process-doc
	./process-doc -w -tag < $(INPUT) > json_tags.rs

types.spec.yaml: $(INPUT) process-doc
	./process-doc -w -yaml < $(INPUT) > types.spec.yaml

process-doc: *.go go.mod
	go build

install: generate
	mv NEW-FORMAT.md ../../doc
	mv json_tags.rs ../../src
	mv types.spec.yaml ../../doc
