.PHONY: default all
# Note _FORTIFY_SOURCE requires -O1 or better
# Note _XOPEN_SOURCE is required for readlink()
FLAGS=-g -O1 -Wall -Werror -Wno-format-truncation -std=c17 -D_FORTIFY_SOURCE=2 -D_XOPEN_SOURCE=500

default: sonar-daemon-runner

all: sonar-daemon-runner sonar-mock

sonar-daemon-runner: sonar-daemon-runner.c protocol.c protocol.h Makefile
	$(CC) $(FLAGS) -DLOGGING -o sonar-daemon-runner sonar-daemon-runner.c protocol.c

sonar-mock: sonar-mock.c protocol.c protocol.h Makefile
	$(CC) $(FLAGS) -o sonar-mock sonar-mock.c protocol.c

format:
	if [[ -n $$(command -v clang-format) ]]; then \
		clang-format -i --style=file:../clang-format.txt *.c *.h; \
	fi
