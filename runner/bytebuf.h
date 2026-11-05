#ifndef bytebuf_h_included
#define bytebuf_h_included

#include <stddef.h>
#include <stdint.h>

/* A bytebuf_t holds a growable array of bytes.  It is in one of three states: new, initialized, and
 * filled.  A buffer in the new state must be initialized before being used.  A buffer must be in
 * the new or initialized state when going out of scope, never in the filled state (or it leaks).
 *
 * A stack or heap allocated bytebuf_t (even if allocated with calloc) starts in the "new" state.  A
 * static bytebuf_t starts in the initialized state.
 */
typedef struct {
    uint32_t len;
    uint32_t cap;
    uint8_t* buf;
} bytebuf_t;

/* The buffer *b must be in the "new" state, it moves to the initialized state. */
void bytebuf_init(bytebuf_t* b);

/* The buffer *b must be in the initialized or filled state, it moves to the initialized state.  If
 * it is in the initialized state then this is a no-op.
 */
void bytebuf_destroy(bytebuf_t* b);

/* The buffer *b must be in the filled state, it moves to the initialized state.  The memory buffer
 * or a portion thereof is returned, it will never be NULL.
 */
uint8_t* bytebuf_take(bytebuf_t* b);

/* Make nbytes available and return a pointer to the first byte.  Returns NULL on OOM.  The buffer b
 * must be in the initialized or filled state, it moves to the filled state (even if nbytes is 0).
 * Note the returned pointer will be invalidated by subsequent calls to all bytebuf methods,
 * including bytebuf_take.
 */
uint8_t* bytebuf_reserve(bytebuf_t* b, size_t nbytes);

/* Append a byte to the buffer (equivalent to bytebuf_reserve + memcpy). */
int bytebuf_append_byte(bytebuf_t* b, uint8_t by);

/* Append some bytes to the buffer (equivalent to bytebuf_reserver + memcpy). */
int bytebuf_append_bytes(bytebuf_t* b, const uint8_t* bytes, size_t nbytes);

#endif
