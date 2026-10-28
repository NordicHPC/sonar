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

/* Grow an array of nelem elements, each of size elemsiz, pointed to by inp.  Inp may be NULL, in
 * which case a new array is allocated.  The new size may be no smaller than the old size, but may
 * be the same.  The new size may be 0 if the old size is 0 or inp is NULL; the return value will be
 * non-NULL in this case.  If nelem*elemsiz overflows ALLOC_MAX or the allocation fails for other
 * reasons, sets errno to ENOMEM and returns NULL.  On a NULL return, the object pointed to by inp
 * has not changed.  Even if the new size is the same as the old size the caller should not rely on
 * the return value being equal to inp.
 *
 * If the array is moved, the contents of the old storage are copied into the new storage.  Any
 * freshly allocated storage is uninitialized.
 */
void* grow_array(void* inp, size_t nelem, size_t elemsiz);

#endif
