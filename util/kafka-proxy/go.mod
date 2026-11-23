module kprox

// Try to keep this at maximally "previous major Go release and at
// least two dot releases behind tip", so as not to depend on bleeding
// edge anywhere.

go 1.26.0

require (
	github.com/lars-t-hansen/ini v0.3.0
	github.com/twmb/franz-go v1.22.3
)

require (
	github.com/klauspost/compress v1.20.5 // indirect
	github.com/pierrec/lz4/v4 v4.1.34 // indirect
	github.com/twmb/franz-go/pkg/kmsg v1.15.1 // indirect
)
