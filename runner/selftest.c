/* This is test code only, not part of the runner */

/* Really you want to run this under valgrind:
 *     valgrind --leak-check=full ./selftest
 */

#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "bytebuf.h"
#include "selftest.h"
#undef read
#undef write

int main(int argc, char** argv) {
    alloc_selftest();
    bytebuf_selftest();
    parse_selftest();
    protocol_selftest();
    exe_for_pids_selftest();
    get_env_var_selftest();
    get_exe_selftest();
}

/* Mocking I/O */
#define NCHAN 2

static struct channel_t {
    int inp;
    bytebuf_t bb;
} channels[NCHAN];

void clear_channel(int c) {
    assert(-c - 1 < NCHAN);
    struct channel_t* buffer = &channels[-c - 1];
    bytebuf_destroy(&buffer->bb);
    buffer->inp = 0;
}

int selftest_read(int input, uint8_t* p, int n) {
    if (input >= 0) {
        return read(input, p, n);
    }
    /* TODO (testing): To improve this: read a max number of bytes per call, say, 5 */
    assert(-input - 1 < NCHAN);
    struct channel_t* buffer = &channels[-input - 1];
    int avail = buffer->bb.len - buffer->inp;
    int k = n > avail ? avail : n;
    memcpy(p, buffer->bb.buf + buffer->inp, k);
    buffer->inp += k;
    return k;
}

int selftest_write(int output, const uint8_t* p, int n) {
    if (output >= 0) {
        return write(output, p, n);
    }
    /* TODO (testing): To improve this: write at most some number of bytes per call, say, 5 */
    assert(-output - 1 < NCHAN);
    struct channel_t* buffer = &channels[-output - 1];
    if (bytebuf_append_bytes(&buffer->bb, p, n) == -1) {
        fprintf(stderr, "Allocation failure\n");
        abort();
    }
    return n;
}
