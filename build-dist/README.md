# Build Sonar RPMs

The configuration assets for Sonar are not bundled in the RPM, so there will typically be one RPM
that is used on all nodes of a particular hardware and OS configuration.

The .spec files here build from source.  You normally need to build on the cluster that will take
the RPM, or on a machine with a compatible OS, to get library versions right.

The build scripts in the .spec files will typically need to be customized for your system, minimally
to specify how to initialize dependencies (Rust, C, Binutils, GPU headers).  There are instructions
in the .spec files.


## RPM cookbook on a Fedora workstation

For the sake of simplicity:
```
$ sudo dnf install rpmdevtools rpmlint
$ rpmdev-setuptree
```

This creates `~/rpmbuild`, which will hold the RPM assets.  Then:

```
$ cp sonar-allgpu.spec ~/rpmbuild/SPECS
```

and then modify the copy according to instructions in the file, and download and copy assets as
specified there.

Now to build:
```
$ rpmbuild -bb ~/rpmbuild/SPECS/sonar-allgpu.spec
```

The other .spec file here, `sonar-nvidia.spec`, is the same but instead of using the pre-created GPU
shim assets that are in the Sonar repo it rebuilds the NVIDIA GPU shim from source and only links
with that.  While the -allgpu RPM will be able to probe NVIDIA, AMD, Habana and XPU GPUs, the -nvidia
GPU will only be able to probe NVIDIA GPUs.


## Older systems

RPM builds on older systems may need to tweak the RPM specs a little since the tools do not always
do everything they need to do for RPM builds to work.  For example, for RHEL9, the `%autorelease`
macro does not exist so the `Release` header of the spec must be changed to look like this:

```
Release:        1%{?dist}
```

Also, the invocation of `cargo build` needs to pass a flag to create a build ID section because that
does not happen automatically:

```
SONAR_CUSTOM_GPUAPI=$SHIMDIR \
RUSTFLAGS="-C link-arg=-Wl,--build-id" \
cargo build --no-default-features --features=daemon,kafka,nvidia --profile=release-with-debug
```

For those with access to the NRIS gitlab, look in `larstha/sonar-deploy/sonar/saga.sigma2.no` for
the .spec for an RHEL9 system.
