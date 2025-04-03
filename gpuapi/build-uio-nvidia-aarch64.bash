#!/usr/bin/env bash
#
# See Makefile for information.
#
# UiO does not have aarch64 nodes with GPUs, so for now we just build the stub library.

set -e
if [[ ! ( $(hostname) =~ freebio.*\.hpc\.uio\.no ) ]]; then
    echo "Wrong host!"
    exit 1
fi
module purge
module load GCC/13.2.0
module list
make libsonar-nvidia-stub.a
mkdir -p aarch64
mv libsonar-nvidia-stub.a aarch64/libsonar-nvidia.a
