# Sonar developer guide

This document collects various facts around how to build and maintain Sonar.  Deployment is covered
in [HOWTO-DEPLOY.md](HOWTO-DEPLOY.md).


## Build Requirements

Sonar is written primarily in Rust, with some support code for GPU access in C and some helper
programs and test code in Go.

At this time we require:

- Linux
- 2021 edition of Rust or newer
- Rust 1.81.0 or newer (can be found with `cargo msrv find`)
- OpenSSL development libraries / headers for Kafka support installed on the build system
  (on Ubuntu, this is libssl-dev, on Red Hat it is openssl-devel)
- A "new enough" binutils (2.35 or newer is known to work, 2.30 is too old, 2.32 is unclear),
  this can be an issue on RHEL8 and similarly old systems. Check with `as --version`

To rebuild the GPU shims (which are included precompiled for some CPU architectures in
subdirectories of `gpuapi/`) you need a recent C compiler (gcc11 is fine) and you must compile on a
host that has the GPU libraries / headers installed.

To run some of the selftests, or to build the Kafka HTTP proxy, or to rebuild the artifacts built
from the formal specification of the output data, you will need a recent version of Go, we strive to
require no more than the previous major release and two dot releases behind tip on that (see
https://go.dev/doc/devel/release); older versions may work.  At the time of writing, the Kafka HTTP
proxy requires Go 1.24 and other programs require Go 1.22.


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

The GPU shims are compiled separately (requires a C compiler) and have to be compiled in an
environment where the GPU headers are installed.  In `gpuapi/Makefile` there are instructions about
how to do that, and in `gpuapi/` there are many shell scripts that are examples of how to do it in
various environments.

### RPM builds

TODO.  To be written for #493.  Things will appear in ../build-dist.  I think I want the generic
sonar-all.spec file there and also the shared assets that are currently in sonar-dist, they are
neither secret nor NRIS-specific.

### Regenerating what is generated

Some code is generated from other code; notably, version number files for some of the auxiliary
artifacts are generated from `Cargo.toml`, and documentation and some Rust code is generated from
the Sonar data format specification.  To regenerate these, run `make generate` at the top level
(requires Go to process the output format specification).


## Very high level code structure

The major components of Sonar are:

* a command line interface in `src/main.rs`
* individual Sonar operations in `src/{cluster,ps,sysinfo,slurmjobs}.rs` (where `ps` implements
  the `sample` command and `slurmjobs` the `jobs` command; these are old names)
* the memory-resident daemon mode in `src/daemon.rs`
* data sinks in `src/datasink/{directory,kafka,stdio}.rs`
* GPU interface definition in `src/gpu/mod.rs`
* GPU interface implementations in `src/gpu/{amd,habana,nvidia,xpu}.rs`
* GPU SMI shim libraries in `gpuapi/sonar-{amd,habana,nvidia,xpu}.{c,h}`
* platform interface definition in `src/systemapi.rs` and a Linux implementation in `src/linux/system.rs`

In addition, external to Sonar proper there are these components:

* A Go definition (types + normative doc comments) of the output format in `util/formats/newfmt/types.go`
* A Kafka HTTP proxy implementation in `util/kafka-proxy/kprox.go`
* A test suite in `test/`

Everything else is utility code, formatting code, and so on.  Not everything is completely well
structured, Sonar has grown tremendously at several hands over several years and sometimes the code
bears the mark of being a prototype-in-progress.


## Portability

Sonar is Linux-only and is known to work on systems at least as old as RHEL8 and as new as Fedora
43; it has been used at various times on both Red Hat and Ubuntu distros.  It does not use exotic
aspects of the kernel or specific distros.

We believe to be possible to port Sonar to at least BSD Unix, and the system API abstraction is set
up to make this easy.  However, we have not attempted such a port.


## Versions and release process

### Version numbers

The following basic versioning rules are new with v0.8.0.

We use semantic versioning.  The major version is expected to remain at zero for the foreseeable
future, reflecting the experimental nature of Sonar.

The minor version is updated with changes that alter the output format deliberately: fields are
added, removed, or are given a new meaning (this has been avoided so far), or the record format
itself changes.  For example, v0.8.0 both added fields and stopped printing fields that are zero.

The minor version is also updated with major internal changes that could destabilize Sonar, thus
stabilizing the previous minor version.

The bugfix version is updated for changes that do not alter the output format per se but that might
affect the output nevertheless, ie, most changes not covered by changes to the minor version number.

### Release branches, tags, uplifts, and backports

#### Branches

The following branching scheme is new with v0.12.x, later refined a bit.

The `main` branch is used for development and has a version number of the form `M.N.O-devel`.  Note
that this version number form will also be present in the output of Sonar commands, to properly tag
those data.  If clients are exposed to prerelease data they must be prepared to deal with this.

When a stablization tag is created on main it the version number is temporarily changed to
`M.N.O-preK` where K increases linearly from 1, but then main itself reverts to the `M.N.O-devel`
tag.

For every freeze of the the minor release number, a new release branch is created in the repo with
the name `release_<major>_<minor>`, again we expect `<major>` to remain `0` for the foreseeable
future, ergo, `release_0_12` is the v0.12.x release branch.  At branching time, the minor release
number is incremented on main (so when we created `release_0_12` for v0.12.1, the version number on
`main` went to `0.13.0-devel`).

Release candidates on a release branch are given the version number `M.N.O-rcK` along with a
corresponding tag.

Actual releases on a release branch have a version number of the form `M.N.O`.

#### Tagging

When a release `M.N.O` (or indeed `M.N.O-preK` or `M.N.O-rcK`) is to be made from a release branch,
a primary version tag must be created of the form `vM.N.O` (`vM.N.O-preK` or `vM.N.O-rcK`) on that
branch and the release is built from that changeset.  Once the release has shipped, the bugfix
version number on the branch is incremented.

Additionally, the tag `util/formats/vM.N.O` (`util/formats/vM.N.O-preK` or
`util/formats/vM.N.O-rcK`) must be created in order for the Go modules system to be able to
reference that version of the Go package `util/formats` (inside Sonar) from other programs.

After tags have been pushed for a proper release (not pre-release or release candidate), the
`vM.N.O` tag should be marked as a release in the Github UI.

#### Uplifts and backports

With the branches come some additional rules for how to move patches around:

- If a bugfix is made to any release branch and the bug is present on main then the PR shall be
  labelled "uplift-required"; the PR shall subsequently be uplifted main; and following uplift the
  label shall be changed to "uplifted-to-main".
- If a bugfix is made to main it shall be considered whether it should be backported the most recent
  release branch.  If so, the PR shall be labelled "backport-required"; the PR shall subsequently be
  cherry-picked or backported to the release branch; and following backport the label shall be
  changed to "backported-to-release".  No older release branches shall automatically be considered
  for backports.


### Policies for changing Rust edition and minimum Rust version

Policy for changing the minimum Rust version:
- Open a GitHub issue and motivate the change
- Once we reach agreement in the issue discussion:
  - Update the version inside the test workflow [test-minimal.yml](.github/workflows/test-minimal.yml)
  - Update the documentation (this section)


### Release pre-testing

Not all testing can be automated because sometimes testing needs to be performed on a range of
different hardware.  Generally this amounts to running `make test` on the various node types.  See
[tests/README.md](../tests/README.md) for more about that.


## Dependencies and supply-chain security

Sonar runs everywhere and all the time, and even though it currently runs without privileges we have
strived to have as few dependencies as possible, so as not to let Sonar become a target of a supply
chain attack.  We made some rules:

- It's OK to depend on libc and to incorporate new versions of libc
- It's better to depend on something from the rust-lang organization than on something else
- Every dependency needs to be justified
- Every dependency must have a compatible license
- Every dependency needs to be vetted as to active development, apparent quality, test cases
- Every dependency update - even for security issues - is to be considered a code change that needs review
- Remember that indirect dependencies are dependencies for us, too, and need to be treated the same way
- If in doubt: copy the parts we need, vet them thoroughly, and maintain them separately

There is a useful discussion of these matters [here](https://research.swtch.com/deps).

*The reality is unfortunately different.*  We had to give up on any meaningful control of the supply
chain around v0.14 as the introduction of the Kafka library required the introduction of a large
number of crates we must trust on faith.  (Users who don't need Kafka can remove it and likely will
see the number of dependencies drop significantly.)

Alas, leaning into the reality of non-control with dependencies, we have since added yet more
dependencies for multi-threading channels and base64 encoding, which may themselves have added
further dependencies.  With that in mind, we should probably go back to using all-singing
all-dancing crates such as Clap (the dependency on which was removed in an effort to control the
supply chain as well as compiled code size), and we should switch the daemon config file format to
TOML so that we can use a pre-existing TOML parser.


## Bindgen

We do not currently use [bindgen](https://github.com/rust-lang/rust-bindgen) to generate the Rust/C
interface to the GPU shims, but we should.  (The interfaces are manually maintained, which was fine
when there were two GPU types but now there are four plus the "fake" GPU and there's some room for
error in doing this manually, in addition to the tedium.)  This change will effectively introduce a
build dependency on `clang-devel`.
