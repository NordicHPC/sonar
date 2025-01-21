# This Makefile is used to build libsonar-nvidia.a and libsonar-amd.a, archives containing code
# callable from Rust that will be statically linked into sonar and will support dynamically loaded
# GPU management libraries.
#
# The .a files must be built on systems that have the SDKs for the various GPUs available; in
# practice, libsonar-amd.a is built on a system with an AMD GPU and toolchain, and libsonar-nvidia.a
# is built on a system with an NVIDIA GPU and toolchain.  For this reason the .a files are checked
# into the git repo, they cannot easily be rebuilt on CI and on random developer systems.
#
# In practice, to prepare for building these libraries on my development systems I currently do this:
#
#   (nvidia)  module load CUDA/11.1.1-GCC-10.2.0
#   (amd)     module load hipSYCL/0.9.2-GCC-11.2.0-CUDA-11.4.1
#
# The .a files must exist in this directory when `cargo build` is run, the linker will look for them
# here.
#
# IMPORTANT:
#
# The .a files and the .o files in them *must* be created on systems that have binutils 2.32 or
# newer.  Most systems do, but eg plain RHEL9 does not - it (currently) has binutils 2.30 by default
# (along with gcc8.5).  On plain RHEL9 systems used for building the .a files or for building Sonar
# it is therefore necessary to upgrade binutils in some way or to `module load` something that
# forces this upgrade.  For example, newer gcc come with newer binutils, so upgrading gcc upgrades
# binutils.
#
# Loading the GPU modules as described above will take care of upgrading binutils appropriately on
# the GPU systems.  To build Sonar when the libraries already exist, it is sufficient to load a
# newer GCC on any host, no GPU toolchains are needed:
#
#   (build)   module load GCC/11.3.0
#
# TODO:
#
# - There's an assumption here that there is only one architecture, this is going to become
#   incorrect before long.  The way to fix this is *probably* to have subdirectories here for the
#   various architectures and to store the artifacts there, and have build.rs push out the
#   appropriate path for the appropriate architectures - however, multi-architecture artifacts may
#   change how we think about it.

CFLAGS=-g -O2 -Wall -fPIC

.PHONY: default

default:
	@echo "Choose a specific target:"
	@echo " libsonar-nvidia.a requires the CUDA SDK to be installed or loaded"
	@echo " libsonar-amd.a requires the ROCM/hip SDK to be installed or loaded"
	@echo ""
	@echo "See comments in Makefile for more information."

libsonar-nvidia.a: sonar-nvidia.o Makefile
	ar rs libsonar-nvidia.a sonar-nvidia.o

sonar-nvidia.o: sonar-nvidia.c sonar-nvidia.h Makefile
	$(CC) -c $(CFLAGS) -o sonar-nvidia.o sonar-nvidia.c

libsonar-amd.a: sonar-amd.o Makefile
	ar rs libsonar-amd.a sonar-amd.o

sonar-amd.o: sonar-amd.c sonar-amd.h Makefile
	$(CC) -c $(CFLAGS) -I/opt/rocm/include -o sonar-amd.o sonar-amd.c
