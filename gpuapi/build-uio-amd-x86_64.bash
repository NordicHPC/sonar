#!/usr/bin/env bash
#
# See Makefile for information.

set -e
if [[ $(hostname) != ml4.hpc.uio.no ]]; then
    echo "Wrong host!"
    exit 1
fi
module load hipSYCL/0.9.2-GCC-11.2.0-CUDA-11.4.1
make libsonar-amd.a
mkdir -p x86_64
mv libsonar-amd.a x86_64
