#ifndef proto_h_included
#define proto_h_included

#include <assert.h>
#include <inttypes.h>

/* Operations that Sonar will send to the server, payload formats etc.
 *
 * Data types:
 *  integer - 4-byte little-endian
 *  string  - integer length followed by utf8 contents
 *  array   - integer length followed by array elements without padding
 *  pid     - integer
 *
 * Message:
 *  integer length of payload, never zero, followed by payload
 *  payload always starts with 1-byte operation code (from set below).
 */

/* In all functions below, a nonzero return means error (and an error message will have been printed
 * on stderr), 0 means success.  The error code is normally 1, but is more generally the exit code
 * if the program chooses to exit.
 */

typedef struct {
    uint32_t len;
    uint8_t  *buf;
    uint8_t  *p;
} inbound_t;

void init_inbound(inbound_t *m);
void destroy_inbound(inbound_t *m);
int decode_byte(inbound_t *m, uint8_t* b);
int decode_int(inbound_t *m, uint32_t *len);

/* On success, *s is a malloc'd NUL-terminated buffer that must be freed */
int decode_string(inbound_t *m, uint8_t** s);

/* The message *m should be in the initialized state. */
int recv_message(int input, inbound_t *m);

typedef struct {
    uint32_t len;
    uint32_t cap;
    uint8_t *buf;
} outbound_t;

void init_outbound(outbound_t *m);
void destroy_outbound(outbound_t *m);
int encode_byte(outbound_t *m, uint8_t b);
int encode_int(outbound_t *m, uint32_t len);
int encode_string(outbound_t *m, const char* s);

/* This will not destroy the message */
int send_message(int output, outbound_t *m);

/* server should exit without waiting for the child.
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
#define REQ_EXE_FOR_PIDS 1

#endif /* proto_h_included */
