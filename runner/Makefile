.PHONY: all
TARGETS=runner subproc

all: $(TARGETS)

runner: runner.o proto.o

subproc: subproc.o proto.o

runner.o subproc.o: *.h

