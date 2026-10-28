#include <assert.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "alloc.h"

void alloc_selftest() {
    uint32_t r;
    assert(alloc_add(10, 20, &r) == 0 && r == 30);
    assert(alloc_add(INT_MAX, 1, &r) == -1);
    assert(alloc_mul(10, 20, &r) == 0 && r == 200);
    assert(alloc_mul(0x20000, 0x7FFF, &r) == -1);
    assert(alloc_mul(UINT32_MAX, UINT32_MAX, &r) == -1);
    void* p;
    assert((p = grow_array(NULL, 2, 8)) != NULL);
    memset(p, 12, 2 * 8);                /* Valgrind will check */
    assert(((char*)p)[2 * 8 - 1] == 12); /* Ditto */
    free(p);
    assert((p = grow_array(NULL, 0, 4)) != NULL);
    assert((p = grow_array(p, 0, 4)) != NULL);
    assert((p = grow_array(p, 1, 4)) != NULL);
    ((uint32_t*)p)[0] = 0x12345678; /* Ditto */
    free(p);
}
