.PHONY: all
TARGETS=runner subproc

default:

runner: runner.c proto.c proto.h Makefile
	$(CC) -g -O0 -DLOGGING -o runner runner.c proto.c

subproc: subproc.c proto.c proto.h Makefile
	$(CC) -g -O0 -o subproc subproc.c proto.c
