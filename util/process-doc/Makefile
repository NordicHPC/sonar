.PHONY: all install

INPUT=../formats/newfmt/types.go

all: NEW-FORMAT.md json_tags.rs

NEW-FORMAT.md: $(INPUT) process-doc
	./process-doc -w -doc < $(INPUT) > NEW-FORMAT.md

json_tags.rs: $(INPUT) process-doc
	./process-doc -w -tag < $(INPUT) > json_tags.rs

process-doc: process-doc.go go.mod
	go build

install: all
	mv NEW-FORMAT.md ../../doc
	mv json_tags.rs ../../src
