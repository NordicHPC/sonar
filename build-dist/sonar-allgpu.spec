# RPM build spec for Sonar for a generic, modern Linux node (with support for all accelerators,
# using the prebuilt GPU shim libraries).  Note that various Linux distros may need tweaks to the
# build setup.  See README.md.
#
# See ../doc/HOWTO-DEVELOP.md regarding build requirements.
#
# Before building:
#
# - place a copy of this file in rpmbuild/SPECS as sonar-<version>-all.spec
# - update the Sonar <version> numbers n.m.k in the copy (two places)
# - wget the appropriate source into rpmbuild/SOURCES
# - modify the build commands below if necessary to set up tools
# - copy rpm-assets/* into rpmbuild/SOURCES/sonar-<version>-assets

Name:           sonar
Version:        0.0.1
Release:        %autorelease
Summary:        Continuous profiling daemon

License:        GPL-3.0 + MIT
URL:            https://github.com/NordicHPC/sonar
Source0:        https://github.com/NordicHPC/sonar/archive/refs/tags/v0.0.1.tar.gz

%description
Sonar is an unprivileged continuous profiling daemon that collects data about jobs, processes,
cores, accelerators, and disks.  It stores the data locally or exfiltrates them to a remote
data collector.

%prep
%setup -q

%build

# See ../doc/HOWTO-DEVELOP.md regarding build requirements.

# Build with default features, ie, support for all cards.
cargo build --profile=release-with-debug

# Assets go into /usr/local/lib/sonar because that makes SELinux happy when running with systemd.
# Sonar always runs as sonar/sonar and gets a home dir in /var/log, so that data can go in there if
# the config wants.

%pre
getent group sonar >/dev/null || groupadd -r sonar
getent passwd sonar >/dev/null || useradd -r -g sonar -d /var/log/sonar -s /sbin/nologin -c "Sonar profiling daemon" sonar

%install
mkdir -p %{buildroot}/usr/local/lib/sonar
mkdir -p %{buildroot}/usr/local/lib/sonar/secrets

# Binary
cp %{_builddir}/sonar-%{version}/target/release-with-debug/sonar %{buildroot}/usr/local/lib/sonar
cp %{_sourcedir}/sonar-%{version}-assets/* %{buildroot}/usr/local/lib/sonar

%files
%dir %attr(755, sonar, sonar) /usr/local/lib/sonar
%attr(755, sonar, sonar) /usr/local/lib/sonar/sonar
%attr(644, sonar, sonar) /usr/local/lib/sonar/sonar.service
%attr(644, sonar, sonar) /usr/local/lib/sonar/sonar.cfg
%attr(644, sonar, sonar) /usr/local/lib/sonar/README
%dir %attr(700, sonar, sonar) /usr/local/lib/sonar/secrets

%changelog
* Fri Feb 06 2026 Lars T Hansen <larstha@uio.no>
- Upstream changelog: https://github.com/NordicHPC/sonar/blob/main/doc/CHANGELOG.md
