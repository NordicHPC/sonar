#ifndef selftest_h_included
#define selftest_h_included

#include <stdint.h>

#ifdef SELFTEST
int selftest_read(int input, uint8_t* p, int n);
int selftest_write(int output, const uint8_t* p, int n);
void clear_channel(int c);

#  define read(x, y, z) selftest_read(x, y, z)
#  define write(x, y, z) selftest_write(x, y, z)

#endif

#endif /* selftest_h_included */
