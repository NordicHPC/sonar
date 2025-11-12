CFLAGS=-Wall -std=c89 -O1
.PHONY: default rollup-programs clean test

default:
	@echo "Pick a specific target"

# Necessary to delete the programs here b/c sometimes the directory is shared between
# machines with different architectures.
test:
	$(MAKE) clean
	./run_tests.sh

rollup-programs: rollup rollup2 rollupchild rollupchild2

format:
	clang-format -i --style=file:../clang-format.txt *.c

rollupchild2: rollupchild
	cp rollupchild rollupchild2

# Same files as in .gitignore
clean:
	rm -rf pincpu rollup rollup2 rollupchild rollupchild2 tmp *~
