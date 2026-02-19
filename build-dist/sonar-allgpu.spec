# RPM build spec for Sonar for a generic, modern Linux node (with support for all accelerators,
# using the prebuilt GPU shim libraries).
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
Version:        0.99.7
Release:        %autorelease
Summary:        Continuous profiling daemon

License:        GPL-3.0 + MIT
URL:            https://github.com/NordicHPC/sonar
Source0:        https://github.com/NordicHPC/sonar/archive/refs/tags/v0.99.7.tar.gz

%description
Sonar is an unprivileged continuous profiling daemon that collects data about jobs, processes,
cores, accelerators, and disks.  It stores the data locally or exfiltrates them to a remote
data collector, under control of a configuration file.

%prep
%setup -q

%build

# See ../doc/HOWTO-DEVELOP.md regarding build requirements.

# Build with default features, ie, support for all cards.
cargo build --profile=release-with-debug

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
