#include <unistd.h>
#include <errno.h>
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
    if ((m->buf + m->len) - m->p < n) {
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

int decode_int(inbound_t *m, uint32_t *n) {
    if (ensure_available(m, 4)) {
        return 1;
    }
    *n = decode_u32(m->p);
    m->p += 4;
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
    m->p += len;
    return 0;
}

/* This reads exactly n bytes and returns 0 if it could.  It returns 1 on error including EOF. */
static int read_bytes(int input, uint8_t* p, int n) {
    int any = 0;
#ifdef LOGGING
    printf("Read entering\n");
#endif
    while (n > 0) {
#ifdef LOGGING
        printf("Read: want %d\n", n);
#endif
        ssize_t m = read(input, p, n);
        printf("Read: %d\n", m);
        if (m == 0) {
            if (any) {
                fprintf(stderr, "Partial message read\n");
            }
#ifdef LOGGING
            printf("EOF\n");
#endif
            return 1;
        }
        if (m == -1) {
            if (errno == EINTR) {
                continue;
            }
            perror("read");
            return 1;
        }
        any = 1;
        p += m;
        n -= m;
#ifdef LOGGING
        printf("Here: %d %d\n", n, m);
#endif
    }
#ifdef LOGGING
    printf("Read returning\n");
#endif
    return 0;
}

int recv_message(int input, inbound_t* m) {
    /* TODO: Make space for the header in a plausibly-sized buffer to be able to make only one call
     * to read() in common cases: most messages will be on the smaller side.
     */
    uint8_t hdr[4];
    if (read_bytes(input, hdr, sizeof(hdr))) {
#ifdef LOGGING
        printf("Reading header failed!!\n");
#endif
        return 1;
    }
    uint32_t nbytes = decode_u32(hdr);
#ifdef LOGGING
    printf("Header: %d\n", nbytes);
#endif
    uint8_t* payload = malloc(nbytes);
    if (payload == 0) {
        perror("malloc");
        return 1;
    }
    if (read_bytes(input, payload, nbytes)) {
#ifdef LOGGING
        printf("Reading payload failed!!\n");
#endif
        free(payload);
        return 1;
    }
#ifdef LOGGING
    printf("Received payload\n");
#endif
    m->len = nbytes;
    m->buf = payload;
    m->p = payload;
    return 0;
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
#ifdef LOGGING
        printf("enough space for %d\n", n);
#endif
        return 0;
    }
#ifdef LOGGING
    printf("must grow for %d\n", n);
#endif
    uint32_t new_cap = m->cap == 0 ? 128 : m->cap;
    while (new_cap - m->len < n && new_cap <= 0x7FFFFFFF) {
#ifdef LOGGING
        printf("new_cap %d\n", new_cap);
#endif
        new_cap *= 2;
    }
    if (new_cap - m->len < n) {
        fprintf(stderr, "Allocation size could not be created for request of %d bytes\n", n);
        return 1;
    }
#ifdef LOGGING
    printf("new_cap = %d\n", new_cap);
#endif
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

static inline void encode_u32(uint8_t* p, uint32_t n) {
    p[0] = n & 255;
    n >>= 8;
    p[1] = n & 255;
    n >>= 8;
    p[2] = n & 255;
    n >>= 8;
    p[3] = n & 255;
}

int encode_int(outbound_t *m, uint32_t n) {
    if (ensure_free(m, 4)) {
        return 1;
    }
    encode_u32(m->buf + m->len, n);
    m->len += 4;
    return 0;
}

int encode_string(outbound_t *m, const char* s) {
    uint32_t len = strlen(s);
    if (ensure_free(m, len + 4)) {
        return 1;
    }
    encode_int(m, len);
    memcpy(m->buf + m->len, s, len);
    m->len += len;
    return 0;
}

static int write_bytes(int output, void* p, size_t n) {
    while (n > 0) {
        ssize_t m = write(output, p, n);
        if (m == -1) {
            perror("write");
            return 1;
        }
        p += m;
        n -= m;
    }
    return 0;
}

int send_message(int output, outbound_t *m) {
    /* TODO: Make space for the header in the buffer to be able to make only one call to write(). */
    if (m->len == 0) {
        return 0;
    }
    uint8_t hdr[4];
    encode_u32(hdr, m->len);
#ifdef LOGGING
    printf("Sending hdr=%d\n", m->len);
#endif
    if (write_bytes(output, hdr, sizeof(hdr))) {
        return 1;
    }
    return write_bytes(output, m->buf, m->len);
}
