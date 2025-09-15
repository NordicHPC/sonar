#!/usr/bin/env bash
#
# See Makefile for information.

set -e
if [[ ! ( $(hostname) =~ h001 ) ]]; then
    echo "Wrong host!"
    exit 1
fi
module purge
module list
make libsonar-habana.a
mkdir -p x86_64
mv libsonar-habana.a x86_64
