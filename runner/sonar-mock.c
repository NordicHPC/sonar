/* Sonar-mock is a test client that stands in for sonar for the purposes of testing the
 * sonar-daemon-runner.  It runs as a client under the sonar-daemon-runner server.  After start, it
 * will generate requests to the server and test that it receives the correct responses.
 *
 * Usage:
 *
 *	sonar-mock -i input-fd -o output-fd "daemon" config-file
 *
 * -i and -o carry the descriptors to use for the pipe (input and output fds) and are required.
 *
 * The word "daemon" must be present literally and we'll check that it is.  The contents of the
 * config-file are ignored but we check that it exists.
 *
 * This could be written in some higher-level language, but by having it written in the same
 * language as the runner we get better testing of the protocol code (probably).  Since the runner
 * is in C, this too is in C.
 */

#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#undef NULL

#include "proto.h"

result_t getint(const char* s, int* n) {
    char* endp;
    errno = 0;
    long k = strtol(s, &endp, 10);
    if (errno != 0 || *endp != 0) {
        return ERR_IO;
    }
    *n = (int)k;
    return OK;
}

int main(int argc, char** argv) {
    argv++;
    if (*argv == nullptr || strcmp(*argv, "-i") != 0) {
        fprintf(stderr, "sonar-mock: Expected -i\n");
        return 1;
    }
    argv++;
    int input = -1;
    if (getint(*argv, &input) != OK) {
        fprintf(stderr, "sonar-mock: Bad -i arg\n");
        return 1;
    }
    argv++;
    if (*argv == nullptr || strcmp(*argv, "-o") != 0) {
        fprintf(stderr, "sonar-mock: Expected -o\n");
        return 1;
    }
    argv++;
    int output = -1;
    if (getint(*argv, &output) != OK) {
        fprintf(stderr, "sonar-mock: Bad -o arg\n");
        return 1;
    }
    argv++;
    if (*argv == nullptr || strcmp(*argv, "daemon") != 0) {
        fprintf(stderr, "sonar-mock: Expected 'daemon'");
        return 1;
    }
    if (*argv == nullptr) {
        fprintf(stderr, "sonar-mock: Expected config-file\n");
        return 1;
    }
#ifdef LOGGING
    const char* config_file = *argv;
#endif
    /* TODO: Check that it exists? */
    argv++;
    if (*argv != nullptr) {
        fprintf(stderr, "sonar-mock: Too many arguments\n");
        return 1;
    }

#ifdef LOGGING
    printf("sonar-mock: %d %d %s\n", input, output, config_file);
#endif
    for (int i = 0; i < 1; i++) {
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
        result_t r = send_message(output, &outbound);
#ifdef LOGGING
        printf("Subproc: sent, sleeping a bit\n");
#endif
        sleep(2);
        destroy_outbound(&outbound);
        if (r != OK) {
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
        // #ifdef LOGGING
        printf("Subproc: received %d\n", inbound.len);
        // #endif
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
        for (int i = 0; i < nelem; i++) {
            uint32_t pid;
            uint8_t* s = nullptr;
            if (decode_int(&inbound, &pid)) {
                printf("Subproc: no pid\n");
                break;
            }
            if (decode_string(&inbound, &s)) {
                printf("Subproc: no path\n");
                break;
            }
            // #ifdef LOGGING
            printf("Subproc: pid=%d path=%s\n", pid, s);
            // #endif
            free(s);
        }
        destroy_inbound(&inbound);
    }
    close(output);
    close(input);
}
