module ingest-kafka

go 1.26.0

require (
	github.com/NordicHPC/sonar/util/formats v0.0.0-00010101000000-000000000000
	github.com/twmb/franz-go v1.22.3
)

require (
	github.com/klauspost/compress v1.20.5 // indirect
	github.com/pierrec/lz4/v4 v4.1.34 // indirect
	github.com/twmb/franz-go/pkg/kmsg v1.15.1 // indirect
)

replace github.com/NordicHPC/sonar/util/formats => ../formats
