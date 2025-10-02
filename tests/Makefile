CFLAGS=-Wall -std=c89
.PHONY: all clean

all: rollup rollup2 rollupchild rollupchild2

format:
	clang-format -i --style=file:../clang-format.txt *.c

rollupchild2: rollupchild
	cp rollupchild rollupchild2

# Same files as in .gitignore
clean:
	rm -rf rollup rollup2 rollupchild rollupchild2 tmp *~
