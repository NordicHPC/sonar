# RPM build spec for binary-only distribution of Sonar for a typical compute node.
#
# The distro is binary-only because the Sonar compilation dependencies are tricky for the build
# hosts (which are usually the cluster login or compute nodes), and in addition the GPU shims must
# be built on systems with the GPU libraries installed.
#
# To use this spec, build your sonar release executable with correct features for the relevant
# architecture and copy it into %%{_sourcedir}/sonar-%%{version}-%%{release}.%%{_arch}/sonar, all
# the other artifacts will also go into that directory:
#
#    README
#    LICENSE
#    (more to come)

Name:           sonar
Version:        0.18.0~pre2
Release:        %autorelease
Summary:        Continuous profiling daemon

License:        GPL-3.0
URL:            https://github.com/NordicHPC/sonar
Source0:        https://github.com/NordicHPC/sonar/archive/refs/tags/v0.18.0-pre2.tar.gz

%description
Sonar is an unprivileged continuous profiling daemon that collects data about jobs, processes,
cores, accelerators, and disks.  It stores the data locally or exfiltrates them to a remote
data collector.

%install
mkdir -p %{buildroot}/var/lib/sonar
cp %{_sourcedir}/sonar-%{version}-%{release}.%{_arch}/sonar %{buildroot}/var/lib/sonar
# More here

%files
/var/lib/%{name}/sonar
#/var/lib/%{name}/sonar-node-kafka.cfg
#/var/lib/%{name}/sonar-node.service

%doc README LICENSE

%changelog
* Fri Feb 06 2026 Lars T Hansen <larstha@uio.no>
- Upstream changelog: https://github.com/NordicHPC/sonar/blob/main/doc/CHANGELOG.md
