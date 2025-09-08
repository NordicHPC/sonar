#ifndef strtcpy_h_included
#define strtcpy_h_included

/* Silently truncating strcpy, returns the number of non-NUL chars copied, always NUL terminates,
   never reads or writes more than necessary. */
static inline size_t strtcpy(char* dest, const char* src, size_t n) {
    size_t m = n;
    while (n > 1 && *src != 0) {
        *dest++ = *src++;
        n--;
    }
    *dest = 0;
    return m-n;
}

#endif
