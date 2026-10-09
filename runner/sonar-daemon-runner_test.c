#include <assert.h>
#include <string.h>
#include <unistd.h>

#include "sonar-daemon-runner.h"

void get_exe_selftest() {
    char buf[PATH_MAX];
    get_exe(getpid(), buf);
    const char* loc = strstr(buf, "selftest");
    assert(loc != NULL);
    assert(loc[8] == 0);
}
