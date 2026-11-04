.PHONY: format test runner clean all

# Default target
runner: build/Makefile
	cmake --build build

format: build/Makefile
	cmake --build build --target format

test: build/Makefile
	ctest --test-dir build --output-on-failure

build/Makefile: CMakeLists.txt
	cmake -B build

clean: build/Makefile
	cmake --build build --target clean
	rm -rf build

