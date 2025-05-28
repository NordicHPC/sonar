#!/usr/bin/env bash
#
# See Makefile for information.

set -e
if [[ ! ( $(hostname) =~ gh001 ) ]]; then
    echo "Wrong host!"
    exit 1
fi
# Build against API 12 to get the maximal API surface.
module purge
module load cuda12.8/toolkit/12.8.1
module list
make libsonar-nvidia.a
mkdir -p aarch64
mv libsonar-nvidia.a aarch64
