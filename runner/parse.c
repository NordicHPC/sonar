#include <errno.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "parse.h"

int parse_longs(const char* input, long** parsed, size_t* num_parsed) {
    *parsed = NULL;
    *num_parsed = 0;

    size_t ix = 0;
    size_t cap = 0;
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
            cap = cap == 0 ? 4 : cap * 2;
            long* new = calloc(cap, sizeof(long));
            if (new == NULL) {
                abort();
            }
            memcpy(new, arr, ix * sizeof(long));
            free(arr);
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
