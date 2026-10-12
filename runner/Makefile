.PHONY: default all format test clean
TARGETS=sonar-daemon-runner sonar-daemon-runner-test sonar-mock selftest godot

# Note _FORTIFY_SOURCE requires -O1 or better
# Note _XOPEN_SOURCE is required for readlink()
# Note _POSIX_C_SOURCE=200809L is intended but does not seem necessary
FLAGS=-g -O1 -Wall -Werror -Wno-format-truncation -std=c17 -D_FORTIFY_SOURCE=2 -D_XOPEN_SOURCE=500

default: sonar-daemon-runner

all: $(TARGETS)

clean:
	rm -f $(TARGETS) *.o test_*.txt *~

sonar-daemon-runner: sonar-daemon-runner.c sonar-daemon-runner.h protocol.c protocol.h Makefile
	$(CC) $(FLAGS) -o sonar-daemon-runner sonar-daemon-runner.c protocol.c

sonar-daemon-runner-test: sonar-daemon-runner.c sonar-daemon-runner.h protocol.c protocol.h Makefile
	$(CC) $(FLAGS) -DLOGGING -o sonar-daemon-runner-test sonar-daemon-runner.c protocol.c

sonar-mock: sonar-mock.c sonar-mock.h protocol.c protocol.h parse.c parse.h Makefile
	$(CC) $(FLAGS) -DLOGGING -o sonar-mock sonar-mock.c protocol.c parse.c

selftest: selftest.c selftest.h \
	parse.c parse.h parse_test.c \
	protocol.c protocol.h protocol_test.c \
	sonar-mock.c sonar-mock.h sonar-mock_test.c \
	sonar-daemon-runner.c sonar-daemon-runner.h sonar-daemon-runner_test.c \
	Makefile
	$(CC) $(FLAGS) -DLOGGING -o selftest -DSELFTEST \
		selftest.c \
		parse.c parse_test.c \
		protocol.c protocol_test.c \
		sonar-mock.c sonar-mock_test.c \
		sonar-daemon-runner.c sonar-daemon-runner.h sonar-daemon-runner_test.c

format:
	if [[ -n $$(command -v clang-format) ]]; then \
		clang-format -i --style=file:../clang-format.txt *.c *.h; \
	fi

test: selftest
	valgrind --leak-check=full --trace-children=yes ./selftest
