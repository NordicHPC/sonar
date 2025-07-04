#!/usr/bin/env bash
#
# See Makefile for information.

set -e
if [[ ! ( $(hostname) =~ n022 ) ]]; then
    echo "Wrong host!"
    exit 1
fi
module purge
module list
make libsonar-xpu.a
mkdir -p x86_64
mv libsonar-xpu.a x86_64
