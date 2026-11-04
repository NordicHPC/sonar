cmake_minimum_required(VERSION 3.50)

project(SonarDaemonRunner)

# Note _FORTIFY_SOURCE requires -O1 or better
# Note _DEFAULT_SOURCE is required for initgroups(), reallocarray()
# Note _POSIX_C_SOURCE is somehow implied hence not specified
set(gccopt -g -O1 -Wall -Werror -Wno-format-truncation -Wpedantic -std=c17)
set(srcopt _FORTIFY_SOURCE=2 _DEFAULT_SOURCE)

add_executable(sonar-daemon-runner)
target_compile_definitions(sonar-daemon-runner PRIVATE ${srcopt})
target_compile_options(sonar-daemon-runner PRIVATE ${gccopt})
target_sources(sonar-daemon-runner
  PRIVATE
    alloc.c
    bytebuf.c
    protocol.c
    sonar-daemon-runner.c
)

add_executable(sonar-daemon-runner-test)
target_compile_definitions(sonar-daemon-runner-test PRIVATE LOGGING ${srcopt})
target_compile_options(sonar-daemon-runner-test PRIVATE ${gccopt})
target_sources(sonar-daemon-runner-test
  PRIVATE
    alloc.c
    bytebuf.c
    protocol.c
    sonar-daemon-runner.c
)

add_executable(sonar-mock)
target_compile_definitions(sonar-mock PRIVATE LOGGING ${srcopt})
target_compile_options(sonar-mock PRIVATE ${gccopt})
target_sources(sonar-mock
  PRIVATE
    alloc.c
    bytebuf.c
    parse.c
    protocol.c
    sonar-mock.c
)

add_executable(selftest)
target_compile_definitions(selftest PRIVATE LOGGING SELFTEST ${srcopt})
target_compile_options(selftest PRIVATE ${gccopt} -fsanitize=undefined)
target_link_options(selftest PRIVATE -lubsan)
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

add_custom_target(check COMMAND ${CMAKE_CTEST_COMMAND})
add_dependencies(check selftest)

enable_testing()
add_test(
  NAME    valgrind-selftest
  COMMAND valgrind --leak-check=full --trace-children=yes --error-exitcode=1 ./selftest
)

add_custom_target(format
  COMMAND
    clang-format -i --style=file:${CMAKE_SOURCE_DIR}/../clang-format.txt
    ${CMAKE_SOURCE_DIR}/*.c
    ${CMAKE_SOURCE_DIR}/*.h
)
