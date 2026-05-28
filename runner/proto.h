#ifndef proto_h_included
#define proto_h_included

#include <assert.h>
#include <inttypes.h>

/* Operations that Sonar will send to the server, payload formats etc.  An "array" is always a
 * 2-byte little-endian unsigned length followed by array elements without padding.  A "string" is
 * always a 2-byte little-endian unsigned length followed by UTF8 data.  A "PID" is a nonzero 4-byte
 * little-endian unsigned integer.
 */

typedef uint8_t sr_len_t[2];

static inline unsigned decode_length(sr_len_t x) {
    return (unsigned)x[0] | ((unsigned)x[1] << 8);
}

static inline void encode_length(unsigned l, sr_len_t x) {
    assert(l < 65536);
    x[0] = l & 255;
    x[1] = (l >> 8) & 255;
}

typedef uint8_t sr_pid_t[3];

/* Server should exit without waiting for the child.
 *
 * Request: 1-byte unsigned exit code.
 *
 * Response: Never responds.
 */
#define REQ_EXIT 0

/* Server should report /proc/PID/exe for PIDs.
 *
 * Request: Array of PIDs.
 *
 * Response: Array of PID/string pairs, all PIDs in the request should be represented in this array.
 * Zero-length strings mean "no information for this PID" (eg process exited).
*/
#define REQ_EXE_FOR_PID 1

#endif /* proto_h_included */
