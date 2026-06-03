.PHONY: all
TARGETS=runner subproc
FLAGS=-g -O0 -Wall -Werror -Wno-format-truncation

default:

sonar-daemon-runner: sonar-daemon-runner.c proto.c proto.h Makefile
	$(CC) $(FLAGS) -DLOGGING -o sonar-daemon-runner sonar-daemon-runner.c proto.c

sonar-mock: sonar-mock.c proto.c proto.h Makefile
	$(CC) $(FLAGS) -o sonar-mock sonar-mock.c proto.c

format:
	if [[ -n $$(command -v clang-format) ]]; then \
		clang-format -i --style=file:../clang-format.txt *.c *.h; \
	fi
