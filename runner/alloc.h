/* Allocation abstractions that are safe against inadvertent integer overflow. */

#ifndef alloc_h_included
#define alloc_h_included

#include <stddef.h>
#include <stdint.h>

/* Do not change this */
#define ALLOC_MAX ((uint32_t)0x7FFFFFFF)

/* Compute *result = a + b and return 0 if there's no overflow past ALLOC_MAX.  Return -1 and set
 * errno to ENOMEM if there's an overflow.
 */
int alloc_add(uint32_t a, uint32_t b, uint32_t* result);

/* Compute *result = a * b and return 0 if there's no overflow past ALLOC_MAX.  Return -1 and set
 * errno to ENOMEM if there's an * overflow.
 */
int alloc_mul(uint32_t a, uint32_t b, uint32_t* result);

/* Reallocate an array of nelem elements each of size elemsiz.  The input pointer may be NULL.  If
 * nelem * elemsiz overflows ALLOC_MAX or the allocation fails for other reasons, sets errno to
 * ENOMEM and returns NULL.  On a NULL return, the object pointed to by the input pointer has not
 * changed.
 */
void* realloc_array(void* inp, size_t nelem, size_t elemsiz);

#endif
