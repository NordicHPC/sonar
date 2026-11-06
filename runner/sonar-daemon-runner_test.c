#include <assert.h>
#include <stdlib.h>
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

void get_env_var_selftest() {
    /* For some patterns we can expect to find some strings on any system - PATH, ^SHELL, HOME,
     * USER, ^PS, USER.  But for reliable testing we want to seed the environment instead.  This
     * seeding must be done *outside* the program, because the program reads /proc/self/environ and
     * that is not updated by setenv or putenv.  Thus when selftest is run these mappings should be
     * set up in the environment:
     *
     *   XZAPPAY=xzappayval
     *   ZAPPA_PREFIX=zappaprefixval
     *   SUFFIX_ZAPPA=zappasuffixval
     *   ZAPPA=zappaval
     */

    struct env_var* result = NULL;
    uint32_t nresult = 0;
    int r, found;

    /* Substring match. */
    /* Put the variable we're looking for in the second place to make sure we match against more
     * than the first one. */
    struct var_pat test_substring[] = {
        { 0, 0, "GGG", 3 },
        { 0, 0, "ZAPPA", 5 },
    };
    assert((r = get_env_vars(getpid(), test_substring, 2, &result, &nresult)) == 0);
    found = 0;
    for (uint32_t i=0 ; i < nresult ; i++) {
        if (strstr(result[i].name, "GGG") == NULL) {
            assert(strstr(result[i].name, "ZAPPA") != NULL);
            assert(strlen(result[i].value) > 0);
            assert(strstr(result[i].value, "zappa") != NULL);
            found++;
        }
        free(result[i].name);
    }
    assert(found == 4);
    free(result);
    result = NULL;

    /* Prefix match */
    struct var_pat test_prefix[] = {
        { 1, 0, "ZAPPA", 5 },
    };
    assert((r = get_env_vars(getpid(), test_prefix, 1, &result, &nresult)) == 0);
    found = 0;
    for (uint32_t i=0 ; i < nresult ; i++) {
        const char* q = strstr(result[i].name, "ZAPPA");
        assert(q != NULL);
        assert(q == result[i].name); /* Prefix */
        assert(strcmp(result[i].name, "ZAPPA") == 0 ||
               strcmp(result[i].name, "ZAPPA_PREFIX") == 0);
        assert(strlen(result[i].value) > 0);
        assert(strstr(result[i].value, "zappa") != NULL);
        free(result[i].name);
        found++;
    }
    assert(found == 2);
    free(result);
    result = NULL;

    /* Suffix match */
    struct var_pat test_suffix[] = {
        { 0, 1, "ZAPPA", 5 },
    };
    assert((r = get_env_vars(getpid(), test_suffix, 1, &result, &nresult)) == 0);
    found = 0;
    for (uint32_t i=0 ; i < nresult ; i++) {
        const char* q = strstr(result[i].name, "ZAPPA");
        assert(q != NULL);
        assert(q[5] == 0); /* Suffix */
        assert(strcmp(result[i].name, "ZAPPA") == 0 ||
               strcmp(result[i].name, "SUFFIX_ZAPPA") == 0);
        assert(strlen(result[i].value) > 0);
        free(result[i].name);
        found++;
    }
    assert(found == 2);
    free(result);
    result = NULL;

    /* Prefix and suffix match */
    struct var_pat test_exact[] = {
        { 1, 1, "ZAPPA", 5 },
    };
    assert((r = get_env_vars(getpid(), test_exact, 1, &result, &nresult)) == 0);
    found = 0;
    for (uint32_t i=0 ; i < nresult ; i++) {
        const char* q = strstr(result[i].name, "ZAPPA");
        assert(q != NULL);
        assert(q == result[i].name); /* Prefix */
        assert(q[5] == 0); /* Suffix */
        assert(strcmp(result[i].name, "ZAPPA") == 0);
        assert(strlen(result[i].value) > 0);
        free(result[i].name);
        found++;
    }
    assert(found == 1);
    free(result);
    result = NULL;
}
