#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "alloc.h"

int alloc_add(uint32_t a, uint32_t b, uint32_t* result) {
    uint64_t tmp = (uint64_t)a + (uint64_t)b;
    if (tmp > ALLOC_MAX) {
        errno = ENOMEM;
        return -1;
    }
    *result = (uint32_t)tmp;
    return 0;
}

int alloc_mul(uint32_t a, uint32_t b, uint32_t* result) {
    uint64_t tmp = (uint64_t)a * (uint64_t)b;
    if (tmp > ALLOC_MAX) {
        errno = ENOMEM;
        return -1;
    }
    *result = (uint32_t)tmp;
    return 0;
}

void* grow_array(void* inp, size_t nelem, size_t elemsiz) {
    if (nelem > UINT32_MAX || elemsiz > UINT32_MAX) {
        errno = ENOMEM;
        return NULL;
    }
    uint32_t newsiz;
    if (alloc_mul((uint32_t)nelem, (uint32_t)elemsiz, &newsiz) == -1) {
        return NULL;
    }
    if (newsiz == 0 && inp != NULL) {
        /* This case would otherwise cause reallocarray to free the object. */
        return inp;
    }
    return reallocarray(inp, nelem, elemsiz);
}
