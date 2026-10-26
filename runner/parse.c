/* This is test code only, not part of the runner */

#include <errno.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "alloc.h"
#include "parse.h"

int parse_longs(const char* input, long** parsed, size_t* num_parsed) {
    *parsed = NULL;
    *num_parsed = 0;

    uint32_t ix = 0;
    uint32_t cap = 0;
    long* arr = NULL;
    for (;;) {
        errno = 0;
        char* endp;
        long inp = strtol(input, &endp, 10);
        if (errno != 0 || endp == input) {
            free(arr);
            return 1;
        }
        if (ix == cap) {
            if (cap == 0) {
                cap = 4;
            } else {
                if (alloc_mul(cap, 2, &cap) == -1) {
                    abort();
                }
            }
            long* new = realloc_array(NULL, cap, sizeof(long));
            if (new == NULL) {
                abort();
            }
            arr = new;
        }
        arr[ix++] = inp;
        input = endp;
        if (*input != ',') {
            break;
        }
        input++;
    }
    if (*input != 0) {
        free(arr);
        return 1;
    }

    *parsed = arr;
    *num_parsed = ix;
    return 0;
}

int parse_int(const char* s, int* n) {
    char* endp;
    errno = 0;
    long k = strtol(s, &endp, 10);
    if (errno != 0 || *endp != 0 || endp == s || k < INT_MIN || k > INT_MAX) {
        return 1;
    }
    *n = (int)k;
    return 0;
}
