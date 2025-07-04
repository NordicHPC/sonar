.PHONY: default

default:
	@echo "Pick an explicit target"

clean:
	rm -f sycl-mmul *.o *~

sycl-mmul: sycl-mmul.cpp
	@echo "Remember, you must have loaded intel icx support first, see sycl-mmul.cpp"
	icpx -fsycl -DSYCL_EXT_ONEAPI_MATRIX_VERSION=4 -o $@ $^
