#include <errno.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "protocol.h"
#include "selftest.h"

#ifdef SELFTEST
void protocol_selftest() {
    clear_channel(-1);

    outbound_t outbound;
    init_outbound(&outbound);

    // Whitebox - after init
    assert(outbound.len == 0);
    assert(outbound.cap == 0);
    assert(outbound.buf == NULL);

    // Blackbox
    assert(encode_byte(&outbound, 10) == OK);
    assert(encode_int(&outbound, 0x12141620) == OK);
    assert(encode_string(&outbound, "hello there") == OK);
    assert(encode_byte(&outbound, 11) == OK);
    assert(encode_int(&outbound, 0x13151721) == OK);
    assert(encode_string(&outbound, "goodbye for now") == OK);
    assert(send_message(-1, &outbound) == OK);

    // Whitebox - after filling
    // This assumes various things about encoding.
    assert(outbound.len >= 1 + 4 + 4 + 11 + 1 + 4 + 4 + 15);
    assert(outbound.len <= outbound.cap);
    assert(outbound.buf != NULL);

    destroy_outbound(&outbound);

    // Whitebox - after destroy
    assert(outbound.len == 0);
    assert(outbound.cap == 0);
    assert(outbound.buf == NULL);

    inbound_t inbound;
    init_inbound(&inbound);

    // Whitebox - after init
    assert(inbound.len == 0);
    assert(inbound.buf == NULL);
    assert(inbound.p == NULL);

    // Blackbox
    assert(recv_message(-1, &inbound) == OK);
    uint8_t by;
    uint32_t in;
    uint8_t* str = NULL;
    assert(decode_byte(&inbound, &by) == OK);
    assert(by == 10);
    assert(decode_int(&inbound, &in) == OK);
    assert(in == 0x12141620);
    assert(decode_string(&inbound, &str) == OK);
    assert(strcmp((char*)str, "hello there") == 0);
    free(str);
    str = NULL;
    assert(decode_byte(&inbound, &by) == OK);
    assert(by == 11);
    assert(decode_int(&inbound, &in) == OK);
    assert(in == 0x13151721);
    assert(decode_string(&inbound, &str) == OK);
    assert(strcmp((char*)str, "goodbye for now") == 0);
    free(str);
    str = NULL;

    // Whitebox - after consuming
    assert(inbound.buf + inbound.len == inbound.p);

    // Whitebox - after destroy
    destroy_inbound(&inbound);
    assert(inbound.len == 0);
    assert(inbound.buf == NULL);
    assert(inbound.p == NULL);

    clear_channel(-1);
}
#endif
