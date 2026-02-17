#!/usr/bin/env bash
#
# This does the same job as rpmdev-setuptree and can be used instead of that if it is not available
# on your system.
set -e
mkdir -p ${HOME}/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
