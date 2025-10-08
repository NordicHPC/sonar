#include <assert.h>
#include <stdlib.h>

#include "parse.h"
#include "selftest.h"

#ifdef SELFTEST
void parse_selftest() {
    long* longs;
    size_t nlongs;
    assert(parse_longs("10,20,530", &longs, &nlongs) == 0);
    assert(nlongs == 3);
    assert(longs[0] == 10);
    assert(longs[1] == 20);
    assert(longs[2] == 530);
    free(longs);

    assert(parse_longs("10,20,", &longs, &nlongs) == 1);
    free(longs);
    assert(parse_longs("10,,", &longs, &nlongs) == 1);
    free(longs);
    assert(parse_longs("", &longs, &nlongs) == 1);
    free(longs);
    assert(parse_longs("10.", &longs, &nlongs) == 1);
    free(longs);

    int n;
    assert(parse_int("33", &n) == 0);
    assert(n == 33);
    assert(parse_int("", &n) == 1);
    assert(parse_int(".", &n) == 1);
}
#endif
