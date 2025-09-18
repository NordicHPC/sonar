# This Makefile is used to build {x86_64,aarch64}/libsonar-*.a, gpu-specific archives containing
# code callable from Rust that will be statically linked into sonar and will support dynamically
# loaded GPU management libraries.
#
# The .a files must be built on systems that have the SDKs for the various GPUs available; in
# practice, libsonar-amd.a is built on a system with an AMD GPU and toolchain, libsonar-nvidia.a is
# built on a system with an NVIDIA GPU and toolchain, libsonar-xpu.a is built on a system with an
# XPU and XPU toolchain, and libsonar-habana.a is built on a system with a Habana GPU and toolchain.
# For this reason the .a files are checked into the git repo, they cannot easily be rebuilt on CI
# and on random developer systems.
#
# And of course, for each CPU architecture the build host must be of that architecture as well, and
# the .a files are ultimately stored in architecture-specific subdirectories of this directory.
#
# To support systems where we have no capability to compile with appropriate GPU SDKs, or where the
# GPU simply does not exist (eg XPU on aarch64), this file can also be used to build 
# libsonar-*-stub.a, which have the same interface but no GPU functionality.
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
# newer.  Most systems do, but eg plain RHEL8 does not - it (currently) has binutils 2.30 by default
# (along with gcc8.5).  On plain RHEL8 systems used for building the .a files or for building Sonar
# it is therefore necessary to upgrade binutils in some way or to `module load` something that
# forces this upgrade.  For example, newer gcc come with newer binutils, so upgrading gcc upgrades
# binutils.
#
# Loading the GPU modules as in the build scripts in this directory will take care of upgrading
# binutils appropriately on those build nodes.  To build Sonar when the libraries already exist, it
# is sufficient to load a newer GCC on any host, no GPU toolchains are needed:
#
#   (build)   module load GCC/11.3.0

# -Wformat-truncation is arguably a GCC misfeature.
CFLAGS=-g -O2 -Wall -Wno-format-truncation -fPIC

.PHONY: default clean realclean

default:
	@echo "Choose a specific target, often one of these:"
	@echo ""
	@echo " libsonar-nvidia.a       the CUDA SDK must be installed or loaded"
	@echo " libsonar-nvidia-stub.a"
	@echo " libsonar-amd.a          the ROCM/hip SDK must be installed or loaded"
	@echo " libsonar-amd-stub.a"
	@echo " libsonar-xpu.a          the XPU SDK must be installed or loaded"
	@echo " libsonar-xpu-stub.a"
	@echo " libsonar-habana.a       the Habana SDK must be installed or loaded"
	@echo " libsonar-habana-stub.a"
	@echo ""
	@echo "See comments in Makefile for more information."

clean:
	rm -f *.o *.a *~ nvidia-shell amd-shell xpu-shell habana-shell

realclean:
	$(MAKE) clean
	rm -rf x86_64 aarch64

libsonar-nvidia.a: sonar-nvidia.o Makefile
	ar rs $@ $<

sonar-nvidia.o: sonar-nvidia.c sonar-nvidia.h strtcpy.h Makefile
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


libsonar-xpu.a: sonar-xpu.o Makefile
	ar rs $@ $<

sonar-xpu.o: sonar-xpu.c sonar-xpu.h strtcpy.h Makefile
	$(CC) -c $(CFLAGS) -DSONAR_XPU_GPU -o $@ $<

libsonar-xpu-stub.a: sonar-xpu-stub.o Makefile
	ar rs $@ $<

sonar-xpu-stub.o: sonar-xpu.c sonar-xpu.h Makefile
	$(CC) -c $(CFLAGS) -o $@ $<

libsonar-habana.a: sonar-habana.o Makefile
	ar rs $@ $<

sonar-habana.o: sonar-habana.c sonar-habana.h strtcpy.h Makefile
	$(CC) -c $(CFLAGS) -DSONAR_HABANA_GPU -o $@ $<

libsonar-habana-stub.a: sonar-habana-stub.o Makefile
	ar rs $@ $<

sonar-habana-stub.o: sonar-habana.c sonar-habana.h Makefile
	$(CC) -c $(CFLAGS) -o $@ $<

# Various test code.
#
# Remember that it may be necessary to `module load` things first, see the build-*.bash scripts in
# this directory.

nvidia-shell: nvidia-shell.c sonar-nvidia.c sonar-nvidia.h Makefile
	$(CC) $(CFLAGS) -DSONAR_NVIDIA_GPU -DLOGGING -o nvidia-shell nvidia-shell.c sonar-nvidia.c -ldl

amd-shell: amd-shell.c sonar-amd.c sonar-amd.h Makefile
	$(CC) $(CFLAGS) -DSONAR_AMD_GPU -DLOGGING -I/opt/rocm/include -o amd-shell amd-shell.c sonar-amd.c -ldl

xpu-shell: xpu-shell.c sonar-xpu.c sonar-xpu.h Makefile
	$(CC) $(CFLAGS) -DSONAR_XPU_GPU -DLOGGING -o xpu-shell xpu-shell.c sonar-xpu.c -ldl

habana-shell: habana-shell.c sonar-habana.c sonar-habana.h Makefile
	$(CC) $(CFLAGS) -DSONAR_HABANA_GPU -DLOGGING -o habana-shell habana-shell.c sonar-habana.c -ldl
