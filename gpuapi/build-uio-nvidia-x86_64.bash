#!/usr/bin/env bash
#
# See Makefile for information.

set -e
if [[ ! ( $(hostname) =~ ml[1-3,5-9]\.hpc\.uio\.no ) ]]; then
    echo "Wrong host!"
    exit 1
fi
module load CUDA/11.1.1-GCC-10.2.0
make libsonar-nvidia.a
mkdir -p x86_64
mv libsonar-nvidia.a x86_64
