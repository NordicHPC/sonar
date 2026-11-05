#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "alloc.h"
#include "bytebuf.h"

void bytebuf_selftest() {
    bytebuf_t b;
    bytebuf_init(&b);

    assert(b.len == 0);
    assert(b.cap == 0);
    assert(b.buf == NULL);

    assert(bytebuf_append_byte(&b, 10) == 0);
    assert(bytebuf_append_bytes(&b, (const uint8_t*)"hello there", 11) == 0);
    assert(bytebuf_append_byte(&b, 11) == 0);
    assert(bytebuf_append_bytes(&b, (const uint8_t*)"goodbye for now", 15) == 0);
    uint8_t* p;
    assert((p = bytebuf_reserve(&b, 5)) != NULL);
    memset(p, 32, 5);
    assert(bytebuf_append_byte(&b, 99) == 0);

    assert(b.len >= 1 + 11 + 1 + 15 + 5 + 1);
    assert(b.len <= b.cap);
    assert(b.buf != NULL);

    p = bytebuf_take(&b);

    assert(b.len == 0);
    assert(b.cap == 0);
    assert(b.buf == NULL);

    static const uint8_t expect[] = {
        10,
        'h',
        'e',
        'l',
        'l',
        'o',
        ' ',
        't',
        'h',
        'e',
        'r',
        'e',
        11,
        'g',
        'o',
        'o',
        'd',
        'b',
        'y',
        'e',
        ' ',
        'f',
        'o',
        'r',
        ' ',
        'n',
        'o',
        'w',
        ' ',
        ' ',
        ' ',
        ' ',
        ' ',
        99,
    };

    assert(memcmp(p, expect, sizeof(expect)) == 0);
    free(p);

    static bytebuf_t bb;

    assert(bb.len == 0);
    assert(bb.cap == 0);
    assert(bb.buf == NULL);

    bytebuf_append_byte(&bb, 37);

    assert(bb.len != 0);
    assert(bb.cap != 0);
    assert(bb.buf != NULL);

    bytebuf_destroy(&bb);

    assert(bb.len == 0);
    assert(bb.cap == 0);
    assert(bb.buf == NULL);

    /* Appending 0 yields 0-length buffer */
    bytebuf_append_bytes(&bb, NULL, 0);

    assert(bb.len == 0);
    assert(bb.cap >= 0);
    assert(bb.buf != NULL);

    assert((p = bytebuf_take(&bb)) != NULL);
    free(p);

    assert(bytebuf_reserve(&bb, ALLOC_MAX + 1) == NULL);
    /* It's technically unspecified if this is well-defined.  Here we assume that the length is
       checked before any of the source data are read, but a valid implementation could read and
       copy source data up to min(nbytes, ALLOC_MAX), and only then fail. */
    assert(bytebuf_append_bytes(&bb, NULL, ALLOC_MAX + 1) == -1);
}
