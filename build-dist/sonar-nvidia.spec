# RPM build spec for Sonar for a generic Linux node (with support for NVIDIA accelerators only).
# Note that various Linux distros may need tweaks to the build setup.  See README.md.
#
# See ../doc/HOWTO-DEVELOP.md regarding build requirements.
#
# Before building:
#
# - place a copy of this file in rpmbuild/SPECS as sonar-<version>-all.spec
# - wget the appropriate source into rpmbuild/SOURCES
# - modify the build commands below if necessary to set up tools and libraries

Name:           sonar
Version:        0.18.1
Release:        %autorelease
Summary:        Continuous profiling daemon

License:        GPL-3.0 + MIT
URL:            https://github.com/NordicHPC/sonar
Source0:        https://github.com/NordicHPC/sonar/archive/refs/tags/v0.18.1.tar.gz

%description
Sonar is an unprivileged continuous profiling daemon that collects data about jobs, processes,
cores, accelerators, and disks.  It stores the data locally or exfiltrates them to a remote
data collector, under control of a configuration file.

%prep
%setup -q

%build

# See ../doc/HOWTO-DEVELOP.md regarding build requirements.
# Note that we are rebuilding the GPU shim here so CUDA headers must be present.

# The 'rm' on gpuapi/ARCH and the song and dance with gpushim-rpm are there to ensure that we do not
# accidentally link with pre-existing assets that are currently in the Sonar source repo.  The
# SONAR_CUSTOM_GPUAPI=gpushim-rpm tells the cargo link phase that we want the link path for the gpu
# shim to be gpushim-rpm/ instead of gpuapi/ARCH.

# Build the GPU shim
SHIMDIR=gpushim-rpm
rm -rf $SHIMDIR
mkdir -p $SHIMDIR
cd gpuapi
rm -rf x86_64 aarch64
make libsonar-nvidia.a
mv libsonar-nvidia.a ../$SHIMDIR
cd ..

# Build sonar, it will show up in target/release-with-debug
SONAR_CUSTOM_GPUAPI=$SHIMDIR \
cargo build --no-default-features --features=daemon,kafka,nvidia --profile=release-with-debug

# Assets go into /usr/local/lib/sonar because that makes SELinux happy when running with systemd.
%install
install -p -D -m 0755 \
        -t %{buildroot}/usr/local/lib/sonar \
        %{_builddir}/sonar-%{version}/target/release-with-debug/sonar
install -p -D -m 0644 \
        -t %{buildroot}/usr/local/lib/sonar \
        %{_builddir}/sonar-%{version}/build-dist/rpm-assets/*

%files
/usr/local/lib/sonar

%changelog
* Fri Feb 06 2026 Lars T Hansen <larstha@uio.no>
- Upstream changelog: https://github.com/NordicHPC/sonar/blob/main/doc/CHANGELOG.md
