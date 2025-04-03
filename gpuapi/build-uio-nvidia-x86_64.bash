#!/usr/bin/env bash
#
# See Makefile for information.

set -e
if [[ ! ( $(hostname) =~ ml[1-3,5-9]\.hpc\.uio\.no ) ]]; then
    echo "Wrong host!"
    exit 1
fi
# Build against API 12 to get the maximal API surface.  Use a recent GCC.
module purge
module load CUDA/12.3.0 GCC/11.3.0
module list
make libsonar-nvidia.a
mkdir -p x86_64
mv libsonar-nvidia.a x86_64
