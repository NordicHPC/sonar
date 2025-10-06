CFLAGS=-Wall -std=c89 -O1
.PHONY: default rollup-programs clean

default:
	@echo "Pick a specific target"

rollup-programs: rollup rollup2 rollupchild rollupchild2

rollupchild2: rollupchild
	cp rollupchild rollupchild2

# Same files as in .gitignore
clean:
	rm -rf pincpu rollup rollup2 rollupchild rollupchild2 \
	daemon-interrupt-*.txt \
	daemon-directory-*.txt daemon-directory-data \
	daemon-kafka-*.txt \
	regress-*.txt \
	*~
