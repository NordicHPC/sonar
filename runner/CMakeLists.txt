# CMake 3.31 is what's on github ubuntu-latest by default as of Q4 2026 (Ubuntu 24), and it's
# currently good enough for us.

cmake_minimum_required(VERSION 3.31)

project(SonarDaemonRunner)

# Note _FORTIFY_SOURCE requires -O1 or better
# Note _DEFAULT_SOURCE is required for initgroups(), reallocarray()
# Note _POSIX_C_SOURCE is somehow implied hence not specified
set(CMAKE_C_FLAGS "-g -O1 -Wall -Werror -Wno-format-truncation -Wpedantic -std=c17 -D_FORTIFY_SOURCE=2 -D_DEFAULT_SOURCE")

add_executable(sonar-daemon-runner)
target_sources(sonar-daemon-runner
  PRIVATE
    alloc.c
    bytebuf.c
    protocol.c
    sonar-daemon-runner.c
)

# This is as the release version except with logging.
add_executable(sonar-daemon-runner-test)
target_compile_definitions(sonar-daemon-runner-test PRIVATE LOGGING)
target_sources(sonar-daemon-runner-test
  PRIVATE
    alloc.c
    bytebuf.c
    protocol.c
    sonar-daemon-runner.c
)

add_executable(sonar-mock)
target_compile_definitions(sonar-mock PRIVATE LOGGING)
target_sources(sonar-mock
  PRIVATE
    alloc.c
    bytebuf.c
    parse.c
    protocol.c
    sonar-mock.c
)

add_executable(selftest)
target_compile_definitions(selftest PRIVATE LOGGING SELFTEST)
target_compile_options(selftest PRIVATE -fsanitize=undefined)
target_link_libraries(selftest ubsan)
target_sources(selftest
  PRIVATE
    alloc.c
    alloc_test.c
    bytebuf.c
    bytebuf_test.c
    parse.c
    parse_test.c
    protocol.c
    protocol_test.c
    selftest.c
    sonar-mock.c
    sonar-mock_test.c
    sonar-daemon-runner.c
    sonar-daemon-runner_test.c
)

add_executable(vladimir)
target_sources(vladimir PRIVATE vladimir.c)

add_custom_target(check COMMAND ${CMAKE_CTEST_COMMAND} --verbose)
add_dependencies(check selftest)

enable_testing()
add_test(
  NAME    valgrind-selftest
  COMMAND ../run_selftest.bash
)

add_custom_target(format
  COMMAND
    clang-format -i --style=file:${CMAKE_SOURCE_DIR}/../clang-format.txt
    ${CMAKE_SOURCE_DIR}/*.c
    ${CMAKE_SOURCE_DIR}/*.h
)
