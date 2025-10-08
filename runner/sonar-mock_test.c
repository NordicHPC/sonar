#include <stddef.h>
#include <string.h>

#include "protocol.h"
#include "selftest.h"
#include "sonar-mock.h"

#ifdef SELFTEST
static int req_channel = -2;
static int resp_channel = -1;

static long pids[] = { 12, 786, 568 };
static size_t npids = 3;
static const char* const expect[] = { "pid_12", "pid_768", "pid_568" };
static int ix = 0;

static void sent_hook() {
    inbound_t inbound;
    init_inbound(&inbound);
    assert(recv_message(req_channel, &inbound) == OK);
    uint8_t op;
    assert(decode_byte(&inbound, &op) == OK);
    assert(op == REQ_EXE_FOR_PIDS);
    uint32_t nelem;
    assert(decode_int(&inbound, &nelem) == OK);
    assert(nelem == npids);
    for (size_t i = 0; i < npids; i++) {
        uint32_t pid;
        assert(decode_int(&inbound, &pid) == OK);
        assert(pid == pids[i]);
    }
    destroy_inbound(&inbound);

    outbound_t outbound;
    init_outbound(&outbound);
    assert(encode_byte(&outbound, REQ_EXE_FOR_PIDS) == OK);
    assert(encode_int(&outbound, nelem) == OK);
    for (size_t i = 0; i < npids; i++) {
        assert(encode_int(&outbound, pids[i]) == OK);
        assert(encode_string(&outbound, expect[i]) == OK);
    }
    assert(send_message(resp_channel, &outbound) == OK);
    destroy_outbound(&outbound);
}

static void check(uint32_t pid, const char* s) {
    assert(pids[ix] == pid);
    assert(strcmp(expect[ix], s) == 0);
    ix++;
}

void exe_for_pids_selftest() {
    clear_channel(req_channel);
    clear_channel(resp_channel);
    assert(test_exe_for_pids(pids, npids, req_channel, resp_channel, sent_hook, check) == OK);
    clear_channel(req_channel);
    clear_channel(resp_channel);
}
#endif
