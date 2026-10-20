.PHONY: all generate format test install clean

INPUT=../formats/newfmt/types.go
OUTPUTS=NEW-FORMAT.md json_tags.rs types.spec.yaml huma-api.go

all: $(OUTPUTS)

# `install` is the old name, used here and there.
generate install: process-doc $(OUTPUTS)
	mv NEW-FORMAT.md ../../doc
	mv json_tags.rs ../../src
	mv types.spec.yaml ../../doc
	mv huma-api.go ../formats/generated/sonar_huma
	sed 's/package newfmt/package sonar_huma/' < ../formats/newfmt/decode_cluster.go > ../formats/generated/sonar_huma/decode_cluster.go
	sed 's/package newfmt/package sonar_huma/' < ../formats/newfmt/decode_jobs.go > ../formats/generated/sonar_huma/decode_jobs.go
	sed 's/package newfmt/package sonar_huma/' < ../formats/newfmt/decode_samples.go > ../formats/generated/sonar_huma/decode_samples.go
	sed 's/package newfmt/package sonar_huma/' < ../formats/newfmt/decode_sysinfo.go > ../formats/generated/sonar_huma/decode_sysinfo.go

version.go: ../../Cargo.toml version.bash
	go generate

NEW-FORMAT.md: $(INPUT) process-doc
	./process-doc -w -doc < $(INPUT) > NEW-FORMAT.md

json_tags.rs: $(INPUT) process-doc
	./process-doc -w -tag < $(INPUT) > json_tags.rs

types.spec.yaml: $(INPUT) process-doc
	./process-doc -w -yaml < $(INPUT) > types.spec.yaml

huma-api.go: $(INPUT) process-doc
	./process-doc -w -huma < $(INPUT) > huma-api.go
	go fmt huma-api.go

process-doc: process-doc.go version.go go.mod
	go build

format:
	go fmt

test:
	go test

clean:
	rm -f $(OUTPUTS)
