# Sonar developer guide

## Build Requirements

At this time we require:

- Linux
- 2021 edition of Rust
- Rust 1.81.0 or newer (can be found with `cargo msrv find`)
- OpenSSL development libraries / headers for Kafka support (on Ubuntu, this is libssl-dev,
  on Red Hat it is openssl-devel)
- A "new enough" binutils (2.35 or newer is known to work, 2.30 is too old, 2.32 is unclear),
  this can be an issue on RHEL8 and similarly old systems. Check with `as --version`

Also, to rebuild the GPU shims (which are included precompiled for some CPU architectures in
subdirectories of `gpuapi/`) you need a recent C compiler (gcc11 is fine) and you must compile on a
host that has the GPU libraries / headers installed.

Also, to run some of the selftests, or to build the Kafka HTTP proxy, or to rebuild the artifacts
built from the formal specification of the output data, you will need a recent version of Go, we
strive to require no more than the previous major release and two dot releases behind tip on that
(currently Go 1.24.10); older versions may work.

## Compilation

### Building with pre-existing GPU shims

For casual development, test builds, and some release builds, `cargo build` or `cargo build --release`
or even just `make` at the top level will build Sonar and link it with the pre-built GPU
shims that are included, currently for x86_64 and aarch64 and for NVIDIA, AMD, Intel Habana, and
Intel XPU GPUs.  For the Intel GPUs the aarch64 libraries are just stubs.

The individual GPUs are defined as features in the top-level Cargo.toml and to compile with just
NVIDIA support, say, run `cargo build --no-default-features --features=daemon,kafka,nvidia` for a
typical (debug) setup.

### Rebuilding the GPU shims

The GPU shims are lightweight C wrappers that probe for specific GPUs and load those GPUs' SMI
libraries if found, and provide access to the GPUs' configuration and performance data.  A Sonar
build can contain shims for all sorts of GPUs and will be able to work with those GPUs if they are
installed on the system.

The GPU shims are compiled separately and have to be compiled in an environment where the GPU
headers are installed.  In `gpuapi/Makefile` there are instructions about how to do that, and in
`gpuapi/` there are many shell scripts that are examples of how to do it in various environments.

### Regenerating what is generated

Some code is generated from other code; notably, version number files for some of the auxiliary
artifacts are generated from `Cargo.toml`, and documentation and some rust code is generated from
the Sonar data format specification.  To regenerate these, run `make generate` at the top level
(requires Go).

## Very high level code structure

The major components of Sonar are:

* a command line interface in `src/main.rs`
* individual Sonar operations in `src/{cluster,ps,sysinfo,slurmjobs}.rs`
* the memory-resident daemon mode in `src/daemon.rs`
* data sinks in `src/datasink/{directory,kafka,stdio}.rs`
* GPU interface definition in `src/gpu/mod.rs`
* GPU interface implementations in `src/gpu/{amd,habana,nvidia,xpu}.rs`
* GPU SMI shim libraries in `gpuapi/sonar-{amd,habana,nvidia,xpu}.{c,h}`
* platform interface definition in `src/systemapi.rs` and a Linux implementation in `src/linux/system.rs`

In addition, external to Sonar proper there are these components:

* A Go definition (types + comments) of the output format in `util/formats/newfmt/types.go`
* A Kafka HTTP proxy implementation in `util/kafka-proxy/kprox.go`

Everything else is utility code, test code, and so on.  Not everything is completely well
structured, Sonar has grown tremendously at several hands over several years and sometimes the code
bears the mark of being a prototype.

## Portability

Sonar is Linux-only and is known to work on systems at least as old as RHEL8 and as new as Fedora
43; it has been used at various times on both Red Hat and Ubuntu distros.  It does not use exotic
aspects of the kernel or specific distros.

We believe to be possible to port Sonar to at least BSD Unix, and the system API abstraction is set
up to make this easy.  However, we have not attempted such a port.

## Versions and release process

We use semantic versioning.  The major version is expected to remain at zero for the foreseeable
future, reflecting the experimental nature of Sonar.

The release process is described in [doc/VERSIONING.md](doc/VERSIONING.md).

