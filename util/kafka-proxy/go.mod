module kprox

// Try to keep this at maximally "previous major Go release and at
// least two dot releases behind tip", so as not to depend on bleeding
// edge anywhere.

go 1.24.10

require (
	github.com/lars-t-hansen/ini v0.3.0
	github.com/twmb/franz-go v1.20.6
)

require (
	github.com/klauspost/compress v1.18.2 // indirect
	github.com/pierrec/lz4/v4 v4.1.22 // indirect
	github.com/twmb/franz-go/pkg/kmsg v1.12.0 // indirect
)
