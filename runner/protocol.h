#ifndef protocol_h_included
#define protocol_h_included

#include "bytebuf.h"
#include <inttypes.h>

/* The coding style in the component is that whoever initially generates a result code that is not
 * OK or ERR_EOF must also log the error to stderr.  Subsequent propagation of the error should not
 * normally print additional error messages, unless that is vital for context. */
typedef enum {
    OK, /* Must be 0 */
    ERR_EXHAUSTED,
    ERR_TOOBIG,
    ERR_ALLOC,
    ERR_IO,
    ERR_EOF,
    ERR_EMPTY_MSG,
    ERR_INVALID_OP,
} result_t;

/* Operations that Sonar will send to the server, payload formats etc.
 *
 * Data types:
 *  integer - 4-byte little-endian
 *  string  - integer length followed by utf8 contents
 *  array   - integer length followed by array elements without padding
 *  struct  - fields are packed end-to-end
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
 * Multiple messages can be sent back-to-back on the client->server pipe, the client need not wait
 * for the server to respond.  WARNING: A pipe has limited capacity and the system may deadlock if
 * one side keeps sending messages without interleaving sending activity with reading responses!
 * The reason for this is that the responding side will be unable to send responses until they are
 * consumed, and will stop consuming messages; the sending side may then be unable to send, and will
 * not be able to consume responses.
 *
 * The order of responses is *always* in the order of requests.  Every request has exactly one
 * response, with the same operation code.  If errors are possible then they are encoded in the
 * response, as detailed below.
 */

/* The inbound message can be in the newly-created, initialized, or filled state */
typedef struct {
    uint32_t len;
    uint8_t* buf;
    uint8_t* p;
} inbound_t;

/* The message *m must be in the newly-created state, it moves to the initialized state. */
void init_inbound(inbound_t* m);

/* The message *m should be in the initialized or filled state, it moves to the initialized
 * state. */
void destroy_inbound(inbound_t* m);

/* Decode functions must only run if *m is in the filled state.  They can fail to allocate or can
 * exhaust the input in the middle of a datum (a malformed message).  Failure leaves the buffer in
 * the filled state but data may or may not have been consumed.
 */
result_t decode_byte(inbound_t* m, uint8_t* b);
result_t decode_int(inbound_t* m, uint32_t* len);

/* On success, *s is a malloc'd NUL-terminated buffer that must be freed */
result_t decode_string(inbound_t* m, uint8_t** s);

/* The message *m should be in the initialized state.  On success, it moves to the filled state.  It
 * can fail to read from the input, or fail to allocate memory.  Failure leaves the buffer in the
 * initialized state.
 */
result_t recv_message(int input, inbound_t* m);

/* The outbound message can be in the newly-created, initialized, or filled state. */
typedef struct {
    bytebuf_t b;
} outbound_t;

/* The message *m must be in the newly-created state, it moves to the initialized state. */
void init_outbound(outbound_t* m);

/* The message *m should be in the initialized or filled state, it moves to the initialized
 * state. */
void destroy_outbound(outbound_t* m);

/* Encode functions must only run if *m is in the initialized or filled state.  They can fail to
 * allocate or can overflow the max buffer size.  Failure leaves the buffer in the filled state but
 * data may not have been added to the buffer. */
result_t encode_byte(outbound_t* m, uint8_t b);
result_t encode_int(outbound_t* m, uint32_t len);
result_t encode_string(outbound_t* m, const char* s);

/* The message must be in the filled state (empty messages cannot be sent).  It will remain in this
 * state.  It can fail to send the message, or the whole message. */
result_t send_message(int output, outbound_t* m);

enum {
    /* server should exit without waiting for the child.
     *
     * Request: 1-byte unsigned exit code.
     *
     * Response: Never responds.
     */
    REQ_EXIT,

    /* Server should report /proc/PID/exe for PIDs.
     *
     * Request: Array(PID).
     *
     * Response: Array({PID, string}), all PIDs in the request will be represented in this
     * array. Zero-length strings mean "no information for this PID" (eg process exited).
     */
    REQ_EXE_FOR_PIDS,

    /* Server should report matching environment variables for PIDs.
     *
     * Request: Array(string) Array(pid): patterns followed by PIDs.  The patterns must be
     * syntactically valid.
     *
     * Response: Array({PID, Array({string,string})), all PIDs in the request will be represented in
     * the outer array in the order of the input array.  A pid may have no matching vars, ie the
     * inner array may be empty.  Each string pair represents a var name and value.
     */
    REQ_ENV_VARS_FOR_PIDS,
};

#endif /* proto_h_included */
