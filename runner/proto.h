#ifndef proto_h_included
#define proto_h_included

#include <assert.h>
#include <inttypes.h>

#ifdef NDEBUG

typedef int result_t;
#  define OK 0
#  define ERR_EXHAUSTED 1
#  define ERR_TOOBIG 1
#  define ERR_ALLOC 1
#  define ERR_IO 1
#  define ERR_EOF 1

#else

struct result_repr_t {
    int n;
};
typedef struct result_repr_t* result_t;
extern result_t OK;
extern result_t ERR_EXHAUSTED;
extern result_t ERR_TOOBIG;
extern result_t ERR_ALLOC;
extern result_t ERR_IO;
extern result_t ERR_EOF;

#endif

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
 *
 * The protocol is that the client sends a message and then the server responds with another
 * message, that is, the opcode is included also in the response though the payload data after
 * the opcode may be different (as documented below).
 *
 * Multiple messages can be sent back-to-back on the client->server pipe, the client need not
 * wait for the server to respond.
 *
 * The order of responses is *always* in the order of requests.  Every request has exactly one
 * response, with the same operation code.  If errors are possible then they are encoded in the
 * response, as detailed below.
 */

/* In all functions below, a nonzero return means error (and an error message will have been printed
 * on stderr), 0 means success.  The error code is normally 1, but is more generally the exit code
 * if the program chooses to exit.
 */

typedef struct {
    uint32_t len;
    uint8_t* buf;
    uint8_t* p;
} inbound_t;

void init_inbound(inbound_t* m);
void destroy_inbound(inbound_t* m);
result_t decode_byte(inbound_t* m, uint8_t* b);
result_t decode_int(inbound_t* m, uint32_t* len);

/* On success, *s is a malloc'd NUL-terminated buffer that must be freed */
result_t decode_string(inbound_t* m, uint8_t** s);

/* The message *m should be in the initialized state. */
result_t recv_message(int input, inbound_t* m);

typedef struct {
    uint32_t len;
    uint32_t cap;
    uint8_t* buf;
} outbound_t;

void init_outbound(outbound_t* m);
void destroy_outbound(outbound_t* m);
result_t encode_byte(outbound_t* m, uint8_t b);
result_t encode_int(outbound_t* m, uint32_t len);
result_t encode_string(outbound_t* m, const char* s);

/* This will not destroy the message */
result_t send_message(int output, outbound_t* m);

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
 * Response: Array of PID/string pairs, all PIDs in the request will be represented in this array.
 * Zero-length strings mean "no information for this PID" (eg process exited).
 */
#define REQ_EXE_FOR_PIDS 1

#endif /* proto_h_included */
