/* Really you want to run this under valgrind:
 *     valgrind --leak-check=full ./selftest
 */

#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "selftest.h"
#undef read
#undef write

void parse_selftest();
void protocol_selftest();
void exe_for_pids_selftest();

int main(int argc, char** argv) {
    parse_selftest();
    protocol_selftest();
    exe_for_pids_selftest();
}

/* Mocking I/O */
#define NCHAN 2

static struct channel_t {
    int inp;
    int outp;
    int cap;
    uint8_t* buf;
} channels[NCHAN];

void clear_channel(int c) {
    assert(-c - 1 < NCHAN);
    struct channel_t* buffer = &channels[-c - 1];
    buffer->inp = 0;
    buffer->outp = 0;
    buffer->cap = 0;
    free(buffer->buf);
    buffer->buf = 0;
}

int selftest_read(int input, uint8_t* p, int n) {
    if (input >= 0) {
        return read(input, p, n);
    }
    /* To improve this: read a max number of bytes per call, say, 5 */
    assert(-input - 1 < NCHAN);
    struct channel_t* buffer = &channels[-input - 1];
    int avail = buffer->outp - buffer->inp;
    int k = n > avail ? avail : n;
    memcpy(p, buffer->buf + buffer->inp, k);
    buffer->inp += k;
    return k;
}

int selftest_write(int output, const uint8_t* p, int n) {
    if (output >= 0) {
        return write(output, p, n);
    }
    /* To improve this: write at most some number of bytes per call, say, 5 */
    assert(-output - 1 < NCHAN);
    struct channel_t* buffer = &channels[-output - 1];
    int avail = buffer->cap - buffer->outp;
    if (n > avail) {
        int newcap = buffer->cap + n * 2;
        uint8_t* new = malloc(newcap);
        memcpy(new, buffer->buf, buffer->outp);
        free(buffer->buf);
        buffer->buf = new;
        buffer->cap = newcap;
    }
    memcpy(buffer->buf + buffer->outp, p, n);
    buffer->outp += n;
    return n;
}
