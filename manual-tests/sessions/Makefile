.PHONY: all clean

PROGRAMS=sonar-session-root sonar-job-root sonar-worker
CXXFLAGS=-O2

all: $(PROGRAMS)
clean:
	rm -f $(PROGRAMS) *.o

sonar-session-root: sonar-session-root.c
sonar-job-root: sonar-job-root.c
sonar-worker: sonar-worker.cc

