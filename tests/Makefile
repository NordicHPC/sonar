CFLAGS=-Wall -std=c89 -O1
.PHONY: default rollup-programs clean test generate

default:
	@echo "Pick a specific target"

# Necessary to delete the programs here b/c sometimes the directory is shared between
# machines with different architectures.
test:
	$(MAKE) clean
	./run_tests.sh

# Add to .gitignore if adding here
ROLLUP_TARGETS=rollup rollup2 daemon-rollup2 rollupchild rollupchild2 \
	drchild0 drchild1 drchild2 drchild3
rollup-programs: $(ROLLUP_TARGETS)

format:
	if [[ -n $$(command -v clang-format) ]]; then \
		clang-format -i --style=file:../clang-format.txt *.c; \
	fi

generate:

rollupchild2: rollupchild
	cp rollupchild rollupchild2

drchild0: rollupchild
	cp rollupchild drchild0

drchild1: rollupchild
	cp rollupchild drchild1

drchild2: rollupchild
	cp rollupchild drchild2

drchild3: rollupchild
	cp rollupchild drchild3

# Same files as in .gitignore
clean:
	rm -rf pincpu pincpus $(ROLLUP_TARGETS) tmp *~
