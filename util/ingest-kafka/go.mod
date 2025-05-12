module ingest-kafka

go 1.22.1

require (
	github.com/NordicHPC/sonar/util/formats v0.0.0-00010101000000-000000000000
	github.com/twmb/franz-go v1.18.1
)

require (
	github.com/klauspost/compress v1.17.11 // indirect
	github.com/pierrec/lz4/v4 v4.1.22 // indirect
	github.com/twmb/franz-go/pkg/kmsg v1.9.0 // indirect
)

replace github.com/NordicHPC/sonar/util/formats => ../formats
