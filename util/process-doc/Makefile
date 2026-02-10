.PHONY: all generate install

INPUT=../formats/newfmt/types.go
OUTPUTS=NEW-FORMAT.md json_tags.rs types.spec.yaml

all: $(OUTPUTS)

generate: version.go $(OUTPUTS)

version.go: ../../Cargo.toml version.bash
	go generate

NEW-FORMAT.md: $(INPUT) process-doc
	./process-doc -w -doc < $(INPUT) > NEW-FORMAT.md

json_tags.rs: $(INPUT) process-doc
	./process-doc -w -tag < $(INPUT) > json_tags.rs

types.spec.yaml: $(INPUT) process-doc
	./process-doc -w -yaml < $(INPUT) > types.spec.yaml

process-doc: process-doc.go version.go go.mod
	go build

install: $(OUTPUTS)
	mv NEW-FORMAT.md ../../doc
	mv json_tags.rs ../../src
	mv types.spec.yaml ../../doc
