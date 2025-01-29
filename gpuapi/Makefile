# This Makefile is used to build libsonar-nvidia.a and libsonar-amd.a, archives containing code
# callable from Rust that will be statically linked into sonar and will support dynamically loaded
# GPU management libraries.
#
# The .a files must be built on systems that have the SDKs for the various GPUs available; in
# practice, libsonar-amd.a is built on a system with an AMD GPU and toolchain, and libsonar-nvidia.a
# is built on a system with an NVIDIA GPU and toolchain.  For this reason the .a files are checked
# into the git repo, they cannot easily be rebuilt on CI and on random developer systems.
#
# And of course, for each CPU architecture the build host must be of that architecture as well, and
# the .a files are ultimately stored in architecture-specific subdirectories of this directory.
#
# To support systems where we have no capability to compile with appropriate GPU SDKs, this file can
# also be used to build libsonar-nvidia-stub.a and libsonar-amd-stub.a, which have the same
# interface but no GPU functionality.
#
# On a particular build node with a particular GPU SDK (or no such SDK), we choose which library to
# build and then build it.  The resulting .a file is then manually moved to the appropriate
# architecture subdirectory.  If we build stub libraries, they must be renamed to their non-stub
# names when moving them.
#
# In this directory are scripts to perform the build process for specific architectures and GPUs on
# specific build nodes at specific sites (eg build-uio-nvidia-x86_64.bash).  Adapt to your needs.
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
# Loading the GPU modules as in the build scripts in this directory will take care of upgrading
# binutils appropriately on those build nodes.  To build Sonar when the libraries already exist, it
# is sufficient to load a newer GCC on any host, no GPU toolchains are needed:
#
#   (build)   module load GCC/11.3.0

CFLAGS=-g -O2 -Wall -fPIC

.PHONY: default clean realclean

default:
	@echo "Choose a specific target:"
	@echo " libsonar-nvidia.a       the CUDA SDK must be installed or loaded"
	@echo " libsonar-nvidia-stub.a"
	@echo " libsonar-amd.a          the ROCM/hip SDK must be installed or loaded"
	@echo " libsonar-amd-stub.a"
	@echo ""
	@echo "See comments in Makefile for more information."

clean:
	rm -f *.o *.a *~

realclean:
	$(MAKE) clean
	rm -rf x86_64 aarch64

libsonar-nvidia.a: sonar-nvidia.o Makefile
	ar rs $@ $<

sonar-nvidia.o: sonar-nvidia.c sonar-nvidia.h Makefile
	$(CC) -c $(CFLAGS) -DSONAR_NVIDIA_GPU -o $@ $<

libsonar-nvidia-stub.a: sonar-nvidia-stub.o Makefile
	ar rs $@ $<

sonar-nvidia-stub.o: sonar-nvidia.c sonar-nvidia.h Makefile
	$(CC) -c $(CFLAGS) -o $@ $<


libsonar-amd.a: sonar-amd.o Makefile
	ar rs $@ $<

sonar-amd.o: sonar-amd.c sonar-amd.h Makefile
	$(CC) -c $(CFLAGS) -DSONAR_AMD_GPU -I/opt/rocm/include -o $@ $<

libsonar-amd-stub.a: sonar-amd-stub.o Makefile
	ar rs $@ $<

sonar-amd-stub.o: sonar-amd.c sonar-amd.h Makefile
	$(CC) -c $(CFLAGS) -o $@ $<
