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
#ifdef LOGGING
        printf("%s\n", *argv);
#endif
        argv++;
    }
#ifdef LOGGING
    printf("Client: %d %d\n", input, output);
#endif
    for (int i=0 ; i< 1; i++) {
        sleep(1);
        outbound_t outbound;
        init_outbound(&outbound);
        encode_byte(&outbound, REQ_EXE_FOR_PIDS);
        encode_int(&outbound, 2);
        encode_int(&outbound, 2038);
        encode_int(&outbound, 156935);
#ifdef LOGGING
        printf("Subproc: sending\n");
#endif
        int r = send_message(output, &outbound);
#ifdef LOGGING
        printf("Subproc: sent, sleeping a bit\n");
#endif
        sleep(2);
        destroy_outbound(&outbound);
        if (r) {
            break;
        }
        inbound_t inbound;
        init_inbound(&inbound);
#ifdef LOGGING
        printf("Subproc: receiving\n");
#endif
        if (recv_message(input, &inbound)) {
            break;
        }
//#ifdef LOGGING
        printf("Subproc: received %d\n", inbound.len);
//#endif
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
        printf("Subproc: %d elements\n", nelem);
        for ( int i= 0; i < nelem; i++ ){
            uint32_t pid;
            uint8_t* s = NULL;
            if (decode_int(&inbound, &pid)) {
                printf("Subproc: no pid\n");
                break;
            }
            if (decode_string(&inbound, &s)) {
                printf("Subproc: no path\n");
                break;
            }
//#ifdef LOGGING
            printf("Subproc: pid=%d path=%s\n", pid, s);
//#endif
            free(s);
        }
        destroy_inbound(&inbound);
    }
    close(output);
    close(input);
}
