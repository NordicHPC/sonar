#!/usr/bin/env bash
#
# See Makefile for information.

set -e
make libsonar-fakegpu.a
mkdir -p x86_64
mv libsonar-fakegpu.a x86_64
