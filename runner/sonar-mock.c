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

#include "protocol.h"

result_t test_exe_for_pids(const char* pids, int input, int output);

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
    if (*argv == NULL || strcmp(*argv, "-i") != 0) {
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
    if (*argv == NULL || strcmp(*argv, "-o") != 0) {
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
    if (*argv == NULL || strcmp(*argv, "daemon") != 0) {
        fprintf(stderr, "sonar-mock: Expected 'daemon'");
        return 1;
    }
    if (*argv == NULL) {
        fprintf(stderr, "sonar-mock: Expected config-file\n");
        return 1;
    }
#ifdef LOGGING
    const char* config_file = *argv;
#endif
    /* TODO: Check that it exists? */
    argv++;
    if (*argv != NULL) {
        fprintf(stderr, "sonar-mock: Too many arguments\n");
        return 1;
    }

#ifdef LOGGING
    printf("sonar-mock: %d %d %s\n", input, output, config_file);
#endif

    char* e;
    if ((e = getenv("REQ_EXE_FOR_PIDS")) != NULL) {
        test_exe_for_pids(e, input, output);
    }
    /* This should cause the parent to exit? */
    close(output);
    close(input);
    return 0;
}

result_t test_exe_for_pids(const char* pids, int input, int output) {
    printf("sonar-mock: exe_for_pids\n");

    request_t request;
    init_request(&request);
    if (encode_byte(&request, REQ_EXE_FOR_PIDS) != OK) {
        abort();
    }
    if (encode_int(&request, 2) != OK) {
        abort();
    }
    int expect = 0;
    for (;;) {
        errno = 0;
        char* endp;
        long pid = strtol(pids, &endp, 10);
        if (errno != 0) {
            fprintf(stderr, "sonar-mock: Bad value in pids string: %s\n", pids);
            return ERR_IO;
        }
        expect++;
        if (encode_int(&request, (int)pid) != OK) {
            abort();
        }
        if (*endp != ',') {
            break;
        }
        pids = endp + 1;
    }
    result_t r = send_message(output, &request);
    destroy_request(&request);
    if (r != OK) {
        fprintf(stderr, "sonar-mock: failed to send\n");
        return ERR_IO;
    }

    response_t response;
    init_response(&response);
    if (recv_message(input, &response)) {
        fprintf(stderr, "sonar-mock: failed to receive\n");
        return ERR_IO;
    }
    uint8_t op;
    if (decode_byte(&response, &op) != OK) {
        fprintf(stderr, "sonar-mock: missing opcode\n");
        return ERR_IO;
    }
    if (op != REQ_EXE_FOR_PIDS) {
        fprintf(stderr, "sonar-mock: bad opcode\n");
        return ERR_IO;
    }
    uint32_t nelem;
    if (decode_int(&response, &nelem) != OK) {
        fprintf(stderr, "sonar-mock: no array length\n");
        return ERR_IO;
    }
    if (nelem != expect) {
        fprintf(stderr, "sonar-mock: bad element count\n");
        return ERR_IO;
    }
    for (int i = 0; i < nelem; i++) {
        uint32_t pid;
        uint8_t* s = NULL;
        if (decode_int(&response, &pid) != OK) {
            fprintf(stderr, "sonar-mock: no pid in result\n");
            return ERR_IO;
        }
        if (decode_string(&response, &s) != OK) {
            fprintf(stderr, "sonar-mock: no string in result\n");
            return ERR_IO;
        }
        printf("sonar-mock: pid=%d path=%s\n", pid, s);
        free(s);
    }
    destroy_response(&response);
    return OK;
}
