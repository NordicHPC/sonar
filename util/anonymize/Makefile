.PHONY: generate format test

anonymize: anonymize.go version.go go.mod ../formats/newfmt/types.go
	go build

generate:
	go generate

format:
	go fmt

test:
	go test
