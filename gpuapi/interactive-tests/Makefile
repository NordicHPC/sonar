.PHONY: default clean

default:
	@echo "Pick an explicit target!"

clean:
	rm -f *-mmul *.o *~

sycl-mmul: sycl-mmul.cpp Makefile
	@echo "Remember, you must have loaded intel icx support first, see sycl-mmul.cpp"
	icpx -fsycl -DSYCL_EXT_ONEAPI_MATRIX_VERSION=4 -o $@ $<

cuda-mmul: cuda-mmul.cu Makefile
	@echo "Remember, on the ML nodes, 'module load gcccuda' to load nvcc"
	nvcc -O2 -forward-unknown-to-host-compiler -Wall --std=c++11 -o $@ $<

hip-mmul: hip-mmul.cc Makefile
	hipcc -O2 -Wall --std=c++20 -o $@ $<
