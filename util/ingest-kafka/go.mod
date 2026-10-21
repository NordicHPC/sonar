module ingest-kafka

go 1.25.13

require (
	github.com/NordicHPC/sonar/util/formats v0.0.0-00010101000000-000000000000
	github.com/twmb/franz-go v1.21.7
)

require (
	github.com/klauspost/compress v1.19.2 // indirect
	github.com/pierrec/lz4/v4 v4.1.26 // indirect
	github.com/twmb/franz-go/pkg/kmsg v1.13.1 // indirect
)

replace github.com/NordicHPC/sonar/util/formats => ../formats
