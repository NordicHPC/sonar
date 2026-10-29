/* Sonar-mock is a test client that stands in for sonar for the purposes of testing the
 * sonar-daemon-runner.  It runs as a client under the sonar-daemon-runner server.  After start, it
 * will generate requests to the server and test that it receives the correct responses.
 *
 * Usage:
 *
 *	sonar-mock "daemon" --request-fd req-fd --response-fd resp-fd config-file
 *
 * The options carry the descriptors to use for communication and are required.
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

#include "parse.h"
#include "protocol.h"
#include "sonar-mock.h"

#ifndef SELFTEST
static void exe_for_pid(uint32_t pid, const char* s) {
    printf("sonar-mock: pid=%" PRIu32 " path=%s\n", (unsigned)pid, s);
}

static void env_var_for_pid(uint32_t pid, const char* name, const char* value) {
    printf("sonar-mock: pid=%" PRIu32 " name=%s value=%s\n", (unsigned)pid, name, value);
}

int main(int argc, char** argv) {
    if (getuid() == 0 || getgid() == 0) {
        fprintf(stderr, "sonar-mock: should not run as root!\n");
        return 1;
    }
#  define NGIDS 20
    gid_t gids[NGIDS];
    int ngroups;
    if ((ngroups = getgroups(NGIDS, gids)) < 0) {
        perror("getgroups");
        return 1;
    }
#  ifdef LOGGING
    fprintf(stderr, "sonar-mock: running as uid=%u euid=%u gid=%u egid=%u\n", (unsigned)getuid(),
        (unsigned)geteuid(), (unsigned)getgid(), (unsigned)getegid());
    fflush(stderr);
    fprintf(stderr, "group list\n");
    for (int i = 0; i < ngroups; i++) {
        fprintf(stderr, "  %u\n", (unsigned)gids[i]);
    }
#  endif
    /* should not be in group zero, but check - initgroups() should prevent this */
    int zero = 0;
    for (int i = 0; i < ngroups; i++) {
        if (gids[i] == 0) {
            zero = 1;
        }
    }
    if (zero) {
        /* Try to open a file that belongs to group root that is not world-readable */
        FILE* fp = fopen("/etc/sudoers", "r");
        if (fp == NULL) {
            fprintf(stderr, "In group zero but failed to open sudoers???\n");
        } else {
            fprintf(stderr, "Managed to open sudoers - very bad\n");
            fclose(fp);
            return 1;
        }
    }
    argv++;
    if (*argv == NULL || strcmp(*argv, "daemon") != 0) {
        fprintf(stderr, "sonar-mock: Expected 'daemon'");
        return 1;
    }
    argv++;
    if (*argv == NULL || strcmp(*argv, "--request-fd") != 0) {
        fprintf(stderr, "sonar-mock: Expected --request-fd\n");
        return 1;
    }
    argv++;
    int request_fd = -1;
    if (parse_int(*argv, &request_fd) != 0) {
        fprintf(stderr, "sonar-mock: Bad --request-fd arg\n");
        return 1;
    }
    argv++;
    if (*argv == NULL || strcmp(*argv, "--response-fd") != 0) {
        fprintf(stderr, "sonar-mock: Expected --response-fd\n");
        return 1;
    }
    argv++;
    int response_fd = -1;
    if (parse_int(*argv, &response_fd) != 0) {
        fprintf(stderr, "sonar-mock: Bad --response-fd arg\n");
        return 1;
    }
    argv++;
    if (*argv == NULL) {
        fprintf(stderr, "sonar-mock: Expected config-file\n");
        return 1;
    }
#  ifdef LOGGING
    const char* config_file = *argv;
#  endif
    /* TODO (testing): Check that the config file exists, part of the contract. */
    argv++;
    if (*argv != NULL) {
        fprintf(stderr, "sonar-mock: Too many arguments: %s\n", *argv);
        return 1;
    }

#  ifdef LOGGING
    fprintf(stderr, "sonar-mock: %d %d %s\n", request_fd, response_fd, config_file);
    fflush(stderr);
#  endif

    /* TEST CASE: REQ_EXE_FOR_PIDS */
    char* e;
    if ((e = getenv("REQ_EXE_FOR_PIDS")) != NULL) {
        long* pids;
        size_t npids;
        if (parse_longs(e, &pids, &npids) != 0) {
            fprintf(stderr, "sonar-mock: Bad pid list in REQ_EXE_FOR_PIDS: %s\n", e);
            return 1;
        }
        printf("sonar-mock: exe_for_pids\n");
        if (test_exe_for_pids(pids, npids, request_fd, response_fd, NULL, exe_for_pid) != OK) {
            /* TODO (testing): Print the exact error */
            fprintf(stderr, "sonar-mock: I/O error");
            return 1;
        }
    }

    /* TEST CASE: REQ_ENV_VARS_FOR_PIDS */
    if ((e = getenv("REQ_ENV_VARS_FOR_PIDS")) != NULL) {
        long* pids;
        size_t npids;
        if (parse_longs(e, &pids, &npids) != 0) {
            fprintf(stderr, "sonar-mock: Bad pid list in REQ_EXE_FOR_PIDS: %s\n", e);
            return 1;
        }
        printf("sonar-mock: env_vars_for_pids\n");
        if (test_env_vars_for_pids(pids, npids, request_fd, response_fd, NULL, env_var_for_pid) != OK) {
            /* TODO (testing): Print the exact error */
            fprintf(stderr, "sonar-mock: I/O error");
            return 1;
        }
    }

    /* Exiting (both here and above) should cause the runner parent to exit too.  The Sonar daemon
       never exits, for it to exit is a fatal error.  Hence we sleep a bit here to give the parent
       time to read all the traffic, and then we exit to force the parent to exit too.
    */
#  ifdef LOGGING
    fprintf(stderr, "sonar-mock: sleeping for a bit\n");
    fflush(stderr);
#  endif
    sleep(3);
    return 0;
}
#endif

result_t test_exe_for_pids(const long* pids, size_t npids, int request_fd, int response_fd,
    void (*sent_hook)(), void (*pid_callback)(uint32_t, const char*)) {
    result_t r;

    outbound_t outbound;
    init_outbound(&outbound);
    if ((r = encode_byte(&outbound, REQ_EXE_FOR_PIDS)) != OK) {
        return r;
    }
    if ((r = encode_int(&outbound, (int)npids)) != OK) {
        return r;
    }
    for (size_t i = 0; i < npids; i++) {
        if ((r = encode_int(&outbound, (int)pids[i])) != OK) {
            return r;
        }
    }
    r = send_message(request_fd, &outbound);
    destroy_outbound(&outbound);
    if (r != OK) {
        fprintf(stderr, "sonar-mock: failed to send\n");
        return r;
    }

    if (sent_hook != NULL) {
        sent_hook();
    }

    inbound_t inbound;
    init_inbound(&inbound);
    if ((r = recv_message(response_fd, &inbound)) != OK) {
        fprintf(stderr, "sonar-mock: failed to receive\n");
        return r;
    }
    uint8_t op;
    if ((r = decode_byte(&inbound, &op)) != OK) {
        fprintf(stderr, "sonar-mock: missing opcode\n");
        return r;
    }
    if (op != REQ_EXE_FOR_PIDS) {
        fprintf(stderr, "sonar-mock: bad opcode\n");
        return ERR_IO;
    }
    uint32_t nelem;
    if ((r = decode_int(&inbound, &nelem)) != OK) {
        fprintf(stderr, "sonar-mock: no array length\n");
        return r;
    }
    if (nelem != npids) {
        fprintf(stderr, "sonar-mock: bad element count\n");
        return ERR_IO;
    }
    for (int i = 0; i < nelem; i++) {
        uint32_t pid;
        uint8_t* s = NULL;
        if ((r = decode_int(&inbound, &pid)) != OK) {
            fprintf(stderr, "sonar-mock: no pid in result\n");
            return r;
        }
        if ((r = decode_string(&inbound, &s)) != OK) {
            fprintf(stderr, "sonar-mock: no string in result\n");
            return r;
        }
        pid_callback(pid, (const char*)s);
        free(s);
    }
    destroy_inbound(&inbound);
    return OK;
}

const char* const patterns[] = {
    "^SHELL",
    "HELL",
    "ARG",
    "^PATH$",
    "PATH$",
};

result_t test_env_vars_for_pids(const long* pids, size_t npids, int request_fd, int response_fd,
                                void (*sent_hook)(), void (*var_callback)(uint32_t, const char*, const char*)) {
    result_t r;

    outbound_t outbound;
    init_outbound(&outbound);
    if ((r = encode_byte(&outbound, REQ_ENV_VARS_FOR_PIDS)) != OK) {
        return r;
    }
    uint32_t npatterns = (uint32_t)(sizeof(patterns)/sizeof(patterns[0]));
    if ((r = encode_int(&outbound, npatterns)) != OK) {
        return r;
    }
    for (uint32_t i = 0 ; i < npatterns ; i++ ) {
        if ((r = encode_string(&outbound, patterns[i])) != OK) {
            return r;
        }
    }
    if ((r = encode_int(&outbound, npids)) != OK) {
        return r;
    }
    for (uint32_t i = 0; i < npids; i++) {
        if ((r = encode_int(&outbound, pids[i])) != OK) {
            return r;
        }
    }
    r = send_message(request_fd, &outbound);
    destroy_outbound(&outbound);
    if (r != OK) {
        fprintf(stderr, "sonar-mock: failed to send\n");
        return r;
    }

    if (sent_hook != NULL) {
        sent_hook();
    }

    inbound_t inbound;
    init_inbound(&inbound);
    if ((r = recv_message(response_fd, &inbound)) != OK) {
        fprintf(stderr, "sonar-mock: failed to receive\n");
        return r;
    }
    uint8_t op;
    if ((r = decode_byte(&inbound, &op)) != OK) {
        fprintf(stderr, "sonar-mock: missing opcode\n");
        return r;
    }
    if (op != REQ_ENV_VARS_FOR_PIDS) {
        fprintf(stderr, "sonar-mock: bad opcode\n");
        return ERR_IO;
    }
    uint32_t nelem;
    if ((r = decode_int(&inbound, &nelem)) != OK) {
        fprintf(stderr, "sonar-mock: no array length\n");
        return r;
    }
    if (nelem != npids) {
        fprintf(stderr, "sonar-mock: bad element count\n");
        return ERR_IO;
    }
    for (int i = 0; i < nelem; i++) {
        uint32_t pid;
        uint32_t nvars;
        uint8_t* name = NULL;
        uint8_t* value = NULL;
        if ((r = decode_int(&inbound, &pid)) != OK) {
            fprintf(stderr, "sonar-mock: no pid in result\n");
            return r;
        }
        if ((r = decode_int(&inbound, &nvars)) != OK) {
            fprintf(stderr, "sonar-mock: no vars in result\n");
            return r;
        }
        for (int j=0 ; j < nvars ; j++) {
            if ((r = decode_string(&inbound, &name)) != OK) {
                fprintf(stderr, "sonar-mock: no name in result\n");
                return r;
            }
            if ((r = decode_string(&inbound, &value)) != OK) {
                fprintf(stderr, "sonar-mock: no value in result\n");
                free(name);
                return r;
            }
            var_callback(pid, (const char*)name, (const char*)value);
            free(name);
            free(value);
        }
    }
    destroy_inbound(&inbound);
    return OK;
}
