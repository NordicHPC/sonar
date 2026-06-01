#include <errno.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#undef NULL

#include "proto.h"

#ifndef NDEBUG

static struct result_repr_t OK_ = { 1 };
result_t OK = &OK_;

static struct result_repr_t ERR_EXHAUSTED_ = { 2 };
result_t ERR_EXHAUSTED = &ERR_EXHAUSTED_;

static struct result_repr_t ERR_TOOBIG_ = { 3 };
result_t ERR_TOOBIG = &ERR_TOOBIG_;

static struct result_repr_t ERR_ALLOC_ = { 4 };
result_t ERR_ALLOC = &ERR_ALLOC_;

static struct result_repr_t ERR_IO_ = { 5 };
result_t ERR_IO = &ERR_IO_;

static struct result_repr_t ERR_EOF_ = { 6 };
result_t ERR_EOF = &ERR_EOF_;

#endif

void init_inbound(inbound_t* m) {
    m->buf = m->p = nullptr;
    m->len = 0;
}

void destroy_inbound(inbound_t* m) {
    free(m->buf);
    m->buf = m->p = nullptr;
    m->len = 0;
}

static result_t ensure_available(inbound_t* m, uint32_t n) {
    if ((m->buf + m->len) - m->p < n) {
        fprintf(stderr, "Input buffer could not supply %d bytes\n", n);
        return ERR_EXHAUSTED;
    }
    return OK;
}

result_t decode_byte(inbound_t* m, uint8_t* b) {
    result_t r;
    if ((r = ensure_available(m, 1)) != OK) {
        return r;
    }
    *b = *m->p;
    m->p++;
    return OK;
}

static inline uint32_t decode_u32(uint8_t* p) {
    uint32_t k = p[3];
    k <<= 8;
    k |= p[2];
    k <<= 8;
    k |= p[1];
    k <<= 8;
    k |= p[0];
    return k;
}

result_t decode_int(inbound_t* m, uint32_t* n) {
    result_t r;
    if ((r = ensure_available(m, 4)) != OK) {
        return r;
    }
    *n = decode_u32(m->p);
    m->p += 4;
    return OK;
}

result_t decode_string(inbound_t* m, uint8_t** s) {
    uint32_t len;
    result_t r;
    if ((r = decode_int(m, &len)) != OK) {
        return r;
    }
    if ((r = ensure_available(m, len)) != OK) {
        return r;
    }
    uint8_t* buf = malloc(len + 1);
    if (buf == nullptr) {
        perror("malloc");
        return ERR_ALLOC;
    }
    memcpy(buf, m->p, len);
    buf[len] = 0;
    *s = buf;
    m->p += len;
    return OK;
}

/* This reads exactly n bytes and returns 0 if it could.  It returns 1 on error including EOF. */
static result_t read_bytes(int input, uint8_t* p, int n) {
    int any = 0;
    while (n > 0) {
        ssize_t m = read(input, p, n);
        if (m == 0) {
            if (any) {
                fprintf(stderr, "Partial message read\n");
            }
            return ERR_EOF;
        }
        if (m == -1) {
            if (errno == EINTR) {
                continue;
            }
            perror("read");
            return ERR_IO;
        }
        any = 1;
        p += m;
        n -= m;
    }
    return 0;
}

result_t recv_message(int input, inbound_t* m) {
    /* TODO: Make space for the header in a plausibly-sized buffer to be able to make only one call
     * to read() in common cases: most messages will be on the smaller side.
     */
    uint8_t hdr[4];
    result_t r;
    if ((r = read_bytes(input, hdr, sizeof(hdr))) != OK) {
        return r;
    }
    uint32_t nbytes = decode_u32(hdr);
    uint8_t* payload = malloc(nbytes);
    if (payload == nullptr) {
        perror("malloc");
        return ERR_ALLOC;
    }
    if ((r = read_bytes(input, payload, nbytes)) != OK) {
        free(payload);
        return r;
    }
    m->len = nbytes;
    m->buf = payload;
    m->p = payload;
    return OK;
}

void init_outbound(outbound_t* m) {
    m->len = m->cap = 0;
    m->buf = nullptr;
}

void destroy_outbound(outbound_t* m) {
    free(m->buf);
    m->len = m->cap = 0;
    m->buf = nullptr;
}

static result_t ensure_free(outbound_t* m, uint32_t n) {
    if (m->cap - m->len >= n) {
        return OK;
    }
    uint32_t new_cap = m->cap == 0 ? 128 : m->cap;
    while (new_cap - m->len < n && new_cap <= 0x7FFFFFFF) {
        new_cap *= 2;
    }
    if (new_cap - m->len < n) {
        fprintf(stderr, "Allocation size could not be computed for request of %d bytes\n", n);
        return ERR_TOOBIG;
    }
    uint8_t* new_buf = realloc(m->buf, new_cap);
    if (new_buf == nullptr) {
        perror("realloc");
        return ERR_ALLOC;
    }
    m->cap = new_cap;
    m->buf = new_buf;
    return OK;
}

result_t encode_byte(outbound_t* m, uint8_t b) {
    result_t r;
    if ((r = ensure_free(m, 1)) != OK) {
        return r;
    }
    m->buf[m->len] = b;
    m->len++;
    return OK;
}

static inline void encode_u32(uint8_t* p, uint32_t n) {
    p[0] = n & 255;
    n >>= 8;
    p[1] = n & 255;
    n >>= 8;
    p[2] = n & 255;
    n >>= 8;
    p[3] = n & 255;
}

result_t encode_int(outbound_t* m, uint32_t n) {
    result_t r;
    if ((r = ensure_free(m, 4)) != OK) {
        return r;
    }
    encode_u32(m->buf + m->len, n);
    m->len += 4;
    return OK;
}

result_t encode_string(outbound_t* m, const char* s) {
    uint32_t len = strlen(s);
    result_t r;
    if ((r = ensure_free(m, len + 4)) != OK) {
        return r;
    }
    encode_int(m, len);
    memcpy(m->buf + m->len, s, len);
    m->len += len;
    return OK;
}

static result_t write_bytes(int output, void* p, size_t n) {
    while (n > 0) {
        ssize_t m = write(output, p, n);
        if (m == -1) {
            perror("write");
            return ERR_IO;
        }
        p += m;
        n -= m;
    }
    return OK;
}

result_t send_message(int output, outbound_t* m) {
    /* TODO: Make space for the header in the buffer to be able to make only one call to write(). */
    if (m->len == 0) {
        return OK;
    }
    uint8_t hdr[4];
    encode_u32(hdr, m->len);
    result_t r;
    if ((r = write_bytes(output, hdr, sizeof(hdr))) != OK) {
        return r;
    }
    return write_bytes(output, m->buf, m->len);
}
