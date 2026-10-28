#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "bytebuf.h"

void bytebuf_init(bytebuf_t* b) {
    b->len = b->cap = 0;
    b->buf = NULL;
}

void bytebuf_destroy(bytebuf_t* b) {
    free(b->buf);
    b->len = b->cap = 0;
    b->buf = NULL;
}

uint8_t* bytebuf_take(bytebuf_t* b) {
    assert(b->buf != NULL);
    uint8_t* xs = b->buf;
    b->len = b->cap = 0;
    b->buf = NULL;
    return xs;
}

static int ensure_free(bytebuf_t* b, uint32_t n) {
    if (b->cap - b->len >= n) {
        return 0;
    }
#ifdef SELFTEST
    const uint32_t initial_cap = 4;
#else
    const uint32_t initial_cap = 32;
#endif
    uint32_t new_cap = b->cap == 0 ? initial_cap : b->cap;
    while (new_cap - b->len < n && new_cap <= 0x7FFFFFFF) {
        new_cap *= 2;
    }
    if (new_cap - b->len < n) {
        return -1;
    }
    uint8_t* new_buf = realloc(b->buf, new_cap);
    if (new_buf == NULL) {
        return -1;
    }
    b->cap = new_cap;
    b->buf = new_buf;
    return 0;
}

uint8_t* bytebuf_reserve(bytebuf_t* b, size_t nbytes) {
    if (ensure_free(b, (uint32_t)nbytes) == -1) {
        return NULL;
    }
    uint8_t* p = b->buf + b->len;
    b->len += nbytes;
    return p;
}

int bytebuf_append_byte(bytebuf_t* b, uint8_t c) {
    uint8_t* p = bytebuf_reserve(b, 1);
    if (p == NULL) {
        return -1;
    }
    *p = c;
    return 0;
}

int bytebuf_append_bytes(bytebuf_t* b, const uint8_t* cs, size_t nbytes) {
    uint8_t* p = bytebuf_reserve(b, nbytes);
    if (p == NULL) {
        return -1;
    }
    memcpy(p, cs, nbytes);
    return 0;
}
