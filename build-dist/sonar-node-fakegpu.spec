# RPM build spec for Sonar on a typical HPC compute node with FAKEGPU accelerators (for testing).
#
# The build script below activates gcc, Rust, Binutils, etc as necessary.  YOU MAY NEED TO ALTER
# THESE SETTINGS FOR YOUR SYSTEM.  In general, though, this script will Just Work on a workstation
# with gcc and rust (and the rpm tools) installed.
#
# THIS IS ONLY AN EXAMPLE.  Below, we place fake config files in the RPM.  For a real install, you
# need proper configs, and maybe more.  See README.md in this directory.

Name:           sonar
Version:        0.99.1
Release:        %autorelease
Summary:        Continuous profiling daemon

License:        GPL-3.0
URL:            https://github.com/NordicHPC/sonar
Source0:        https://github.com/NordicHPC/sonar/archive/refs/tags/v0.99.1.tar.gz

%description
Sonar is an unprivileged continuous profiling daemon that collects data about jobs, processes,
cores, accelerators, and disks.  It stores the data locally or exfiltrates them to a remote
data collector.

%prep
%setup -q

%build

# Sonar builds depend on GCC 11 and newer, Binutils 2.35 or newer (should be OK with GCC 11), and
# Rust 1.81.0 or newer.  These have to be activated by the build script below if they are not
# installed on the system and available through the PATH.
#
# Here we assume rust, gcc, and good binutils are all in the path and no further setup is required.
#
# The 'rm' on gpuapi/ARCH and the song and dance with gpushim-rpm are there to ensure that we do not
# accidentally link with pre-existing assets that are currently in the source repo.  The
# SONAR_CUSTOM_GPUAPI=gpushim-rpm tells the cargo link phase that we want the link path for the gpu
# shim to be gpushim-rpm/ instead of gpuapi/ARCH.

# Build the GPU shim
SHIMDIR=gpushim-rpm
rm -rf $SHIMDIR
mkdir -p $SHIMDIR
cd gpuapi
rm -rf x86_64 aarch64
make libsonar-fakegpu.a
mv libsonar-fakegpu.a ../$SHIMDIR
cd ..

# Build sonar, it will show up in target/release
SONAR_CUSTOM_GPUAPI=$SHIMDIR cargo build --no-default-features --features=daemon,kafka,fakegpu --release

%install
mkdir -p %{buildroot}/var/lib/sonar

# Binary
cp %{_builddir}/sonar-%{version}/target/release/sonar %{buildroot}/var/lib/sonar

# Misc assets.  The secrets are not part of the RPM, but must be set up separately.
cp %{_builddir}/sonar-%{version}/build-dist/assets/sonar-fakegpu-node.cfg %{buildroot}/var/lib/sonar
cp %{_builddir}/sonar-%{version}/build-dist/assets/README %{buildroot}/var/lib/sonar

%files
/var/lib/sonar/sonar
/var/lib/sonar/sonar-fakegpu-node.cfg
/var/lib/sonar/README

%changelog
* Fri Feb 06 2026 Lars T Hansen <larstha@uio.no>
- Upstream changelog: https://github.com/NordicHPC/sonar/blob/main/doc/CHANGELOG.md
