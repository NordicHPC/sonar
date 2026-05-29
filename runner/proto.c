#include <inttypes.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "proto.h"

void init_inbound(inbound_t *m) {
    m->buf = m->p = NULL;
    m->len = 0;
}

void destroy_inbound(inbound_t *m) {
    free(m->buf);
    m->buf = m->p = NULL;
    m->len = 0;
}

static int ensure_available(inbound_t *m, uint32_t n) {
    if (m->p - m->buf < n) {
        fprintf(stderr, "Input buffer could not supply %d bytes\n", n);
        return 1;
    }
    return 0;
}

int decode_byte(inbound_t *m, uint8_t* b) {
    if (ensure_available(m, 1)) {
        return 1;
    }
    *b = *m->p;
    m->p++;
    return 0;
}

int decode_int(inbound_t *m, uint32_t *n) {
    if (ensure_available(m, 4)) {
        return 1;
    }
    uint32_t k = m->p[3];
    k <<= 8;
    k |= m->p[2];
    k <<= 8;
    k |= m->p[1];
    k <<= 8;
    k |= m->p[0];
    m->p += 4;
    *n = k;
    return 0;
}

int decode_string(inbound_t *m, uint8_t** s) {
    uint32_t len;
    if (decode_int(m, &len)) {
        return 1;
    }
    if (ensure_available(m, len)) {
        return 1;
    }
    uint8_t* buf = malloc(len+1);
    if (buf == NULL) {
        perror("malloc");
        return 1;
    }
    memcpy(buf, m->p, len);
    buf[len] = 0;
    *s = buf;
    return 0;
}

int recv_message(int input, inbound_t* m) {
    abort();
/*
    uint8_t hdr[4];
    errno = 0;
    // May need to iterate?  Complex.
    int n = read(input, (char*)&hdr, sizeof(hdr));
    if (n == 0) {
        err = "Unexpected EOF";
        return 1;
    }
    if (n < 0) {
        if (errno == EAGAIN) {
            continue;
        }
        perror("server read");
        return 1;
    }
    printf("Server: got msg %d\n", op);
*/
}

void init_outbound(outbound_t *m) {
    m->len = m->cap = 0;
    m->buf = NULL;
}

void destroy_outbound(outbound_t *m) {
    free(m->buf);
    m->len = m->cap = 0;
    m->buf = NULL;
}

static int ensure_free(outbound_t *m, uint32_t n) {
    if (m->cap - m->len >= n) {
        return 0;
    }
    uint32_t new_cap = m->cap == 0 ? 128 : m->cap;
    while (new_cap - m->len < n && new_cap <= 0x7FFFFFFF) {
        new_cap *= 2;
    }
    if (new_cap - m->len < n) {
        fprintf(stderr, "Allocation size could not be created for request of %d bytes\n", n);
        return 1;
    }
    void *new_buf = realloc(m->buf, new_cap);
    if (new_buf == NULL) {
        perror("realloc");
        return 1;
    }
    m->cap = new_cap;
    m->buf = new_buf;
    return 0;
}

int encode_byte(outbound_t *m, uint8_t b) {
    if (ensure_free(m, 1)) {
        return 1;
    }
    m->buf[m->len] = b;
    m->len++;
    return 0;
}

int encode_int(outbound_t *m, uint32_t n) {
    if (ensure_free(m, 4)) {
        return 1;
    }
    uint8_t* p = m->buf + m->len;
    p[0] = n & 255;
    n >>= 8;
    p[1] = n & 255;
    n >>= 8;
    p[2] = n & 255;
    n >>= 8;
    p[3] = n & 255;
    m->len += 4;
    return 1;
}

int encode_string(outbound_t *m, const char* s) {
    uint32_t slen = strlen(s);
    uint32_t len = slen + 4;
    if (ensure_free(m, len)) {
        return 1;
    }
    encode_int(m, len);
    strcpy(m->buf + m->len, s);
    m->len += slen;
    return 0;
}

int send_message(int output, outbound_t *m) {
    abort();
}
