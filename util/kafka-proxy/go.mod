module kprox

// Try to keep this at maximally "previous major Go release and at
// least two dot releases behind tip", so as not to depend on bleeding
// edge anywhere.

go 1.25.13

require (
	github.com/lars-t-hansen/ini v0.3.0
	github.com/twmb/franz-go v1.21.7
)

require (
	github.com/klauspost/compress v1.19.2 // indirect
	github.com/pierrec/lz4/v4 v4.1.26 // indirect
	github.com/twmb/franz-go/pkg/kmsg v1.13.1 // indirect
)
