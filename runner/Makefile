.PHONY: all
TARGETS=runner subproc
FLAGS=-g -O0 -Wall -Werror -Wno-format-truncation

default:

runner: runner.c proto.c proto.h Makefile
	$(CC) $(FLAGS) -DLOGGING -o runner runner.c proto.c

subproc: subproc.c proto.c proto.h Makefile
	$(CC) $(FLAGS) -o subproc subproc.c proto.c

format:
	if [[ -n $$(command -v clang-format) ]]; then \
		clang-format -i --style=file:../clang-format.txt *.c *.h; \
	fi
