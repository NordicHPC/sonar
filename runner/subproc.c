/* This is a test client that stands in for sonar.  The arguments -i and -o carry the descriptors to
 * use for the pipe (input and output fds).
 *
 * There needs to be a timeout on reading from the service, not more than a few seconds probably.
 * If the service times out it should be assumed to be suspec, and if twice it should be assumed to
 * be dead, and an error should be propagated accordingly.
 */

#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <inttypes.h>
#include <stdlib.h>

#include "proto.h"

void msg(const char* s) {
    write(2, s, strlen(s));
}

int main(int argc, char** argv) {
    int input = -1, output = -1;
    while (*argv) {
        if (strcmp(*argv, "-i") == 0) {
            argv++;
            if (*argv == 0) {
                msg("Bad args\n");
                return 1;
            }
            sscanf(*argv, "%d", &input);
            argv++;
            continue;
        }
        if (strcmp(*argv, "-o") == 0) {
            argv++;
            if (*argv == 0) {
                msg("Bad args\n");
                return 1;
            }
            sscanf(*argv, "%d", &output);
            argv++;
            continue;
        }
        printf("%s\n", *argv);
        argv++;
    }
    printf("Client: %d %d\n", input, output);
    for (int i=0 ; i< 10; i++) {
        sleep(1);
        outbound_t outbound;
        init_outbound(&outbound);
        encode_byte(&outbound, REQ_EXE_FOR_PIDS);
        encode_int(&outbound, 2);
        encode_int(&outbound, 12345);
        encode_int(&outbound, 24680);
        int r = send_message(output, &outbound);
        destroy_outbound(&outbound);
        if (r) {
            break;
        }
        inbound_t inbound;
        init_inbound(&inbound);
        if (recv_message(input, &inbound)) {
            break;
        }
        uint8_t op;
        if (decode_byte(&inbound, &op)) {
            break;
        }
        assert(op == REQ_EXE_FOR_PIDS);
        uint32_t nelem;
        if (decode_int(&inbound, &nelem)) {
            break;
        }
        assert(nelem == 2);
        for ( int i= 0; i < nelem; i++ ){
            uint32_t pid;
            uint8_t* s = NULL;
            if (decode_int(&inbound, &pid)) {
                break;
            }
            if (decode_string(&inbound, &s)) {
                break;
            }
            printf("%d: %s\n", pid, s);
            free(s);
        }
        destroy_inbound(&inbound);
    }
}
