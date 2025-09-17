#!/usr/bin/env bash
#
# See Makefile for information.
#
# There do not exist aarch64 nodes with XPU GPUs; always build the stub library.

set -e
if [[ ! ( $(hostname) =~ freebio.*\.hpc\.uio\.no ) ]]; then
    echo "Wrong host!"
    exit 1
fi
module purge
module load GCC/13.2.0
module list
make libsonar-xpu-stub.a
mkdir -p aarch64
mv libsonar-xpu-stub.a aarch64/libsonar-xpu.a
