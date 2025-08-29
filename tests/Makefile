CFLAGS=-Wall -std=c89
.PHONY: all clean

all: rollup rollup2 rollupchild rollupchild2

rollupchild2: rollupchild
	cp rollupchild rollupchild2

# Same files as in .gitignore
clean:
	rm -rf rollup rollup2 rollupchild rollupchild2 \
	daemon-interrupt-*.txt \
	daemon-directory-*.txt daemon-directory-data \
	daemon-kafka-*.txt \
	*~
