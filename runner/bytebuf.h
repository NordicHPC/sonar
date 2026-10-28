#ifndef bytebuf_h_included
#define bytebuf_h_included

#include <stdint.h>

typedef struct {
    uint32_t len;
    uint32_t cap;
    uint8_t* buf;
} bytebuf_t;

/* The buffer *b must be in the newly-created state, it moves to the initialized state. */
void bytebuf_init(bytebuf_t* b);

/* The buffer *b should be in the initialized or filled state, it moves to the initialized
 * state.  If it is in the initialized state then this is a no-op and not necessary.
 */
void bytebuf_destroy(bytebuf_t* b);

/* The buffer *b should be in the filled state, it moves to the initialized state.  The memory
 * buffer is returned, it will never be NULL.
 */
uint8_t* bytebuf_take(bytebuf_t* b);

/* Make nbytes available and return a pointer to the first byte.  Returns NULL on OOM.  The buffer b
 * should be in the initialized or filled state, it moves to the filled state unless nbytes is 0.
 * Note the returned pointer will be invalidated by subsequent calls to all bytebuf methods apart
 * from bytebuf_take.
 */
uint8_t* bytebuf_reserve(bytebuf_t* b, size_t nbytes);

/* Append a byte to the buffer.  Returns 0 on success, -1 on allocation failure. */
int bytebuf_append_byte(bytebuf_t* b, uint8_t by);

/* Append some bytes to the buffer.  Returns 0 on success, -1 on allocation failure. Moves to the
 * filled state unless nbytes is 0. */
int bytebuf_append_bytes(bytebuf_t* b, const uint8_t* bytes, size_t nbytes);

#endif
