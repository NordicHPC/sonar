/* This program is meant to run as root.  It forks off a non-privileged `sonar daemon` and sets
   itself up as a server that will respond to very limited requests for information that is only
   available to root.  Communication is over a pipe: Sonar may send questions and this program will
   respond with an answer.  The protocol is documented in proto.h.

   Usage (as root):

     sonar-daemon-runner path-to-sonar path-to-daemon-config-file user group

   If the child terminates, the server will also terminate (with the same exit code ideally).

   If the child asks for the server to terminate, it will terminate with the passed exit code.

   In principle, the server can time out waiting for payload data, and if so, should terminate with
   an error.

   Notes:

   This needs to be trustworthy, so it is simple and uses solid, believable abstractions to maintain
   that simplicity.
*/

#include <inttypes.h>
#include <string.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <signal.h>
#include <linux/limits.h>

#include "proto.h"

#ifndef PATH_MAX
#  define PATH_MAX 4096
#endif

void sonar(const char* path, const char* config, const char* user, const char* group, int input, int output);
int server(int input, int output);
int get_exe(uint32_t pid, char buf[PATH_MAX]);

int main(int argc, char** argv) {
    if (argc != 5) {
        fprintf(stderr, "Usage: %s path-to-sonar path-to-config user-name group-name\n", argv[0]);
        return 1;
    }
#if 0
    if (getuid() != 0) {
        perror("getuid");
        return 1;
    }
#endif
    /* TODO: check/find user/group */
    /* We use getpwnam_r() to map user -> uid */
    /* Looks like getgrnam_r() to map group -> gid */
    /* Bail early if not obtainable?  Or just handle this in sonar() with the rest? */

    int down[2];
    if (pipe(down) != 0) {
        perror("pipe2");
        return 1;
    }
    int up[2];
    if (pipe(up) != 0) {
        perror("pipe2");
        return 1;
    }

    /* This is wrong.  If the child exits, we should wait() and exit(), and indeed
     * any reading should be aborted.  So if server() gets a read failure it must
     * consider the possibility that the child has exited.
     */
    if (signal(SIGCHLD, SIG_IGN) == SIG_ERR) {
        perror("signal");
        return 1;
    }

    pid_t pid = fork();
    switch (pid) {
    case -1:
        perror("fork");
        return 1;
    case 0:
        close(down[0]);
        close(up[1]);
        return server(up[0], down[1]);
    default:
        close(down[1]);
        close(up[0]);
        sonar(argv[1], argv[2], argv[3], argv[4], down[0], up[1]);
        return 1;
    }
}

void sonar(const char* path, const char* config, const char* user, const char* group, int input, int output) {
    /* TODO: Drop privileges to user/group */
    /* For this, use setuid(), which is safe.  setgid() also looks like the right thing. */
    printf("Sonar: %d %d\n", input, output);
    char ins[20], outs[20];
    sprintf(ins, "%d", input);
    sprintf(outs, "%d", output);
    int r = execl(path, path, "-i", ins, "-o", outs, "daemon", config, (char*)NULL);
    perror("exec");
}

int server(int input, int output) {
    int r = 1;
    inbound_t inbound;
    outbound_t outbound;
    init_inbound(&inbound);
    init_outbound(&outbound);
    for (;;) {
        destroy_inbound(&inbound);
        destroy_outbound(&outbound);
        printf("Runner receiving\n");
        if (!recv_message(input, &inbound)) {
            goto Done;
        }
        printf("Runner received\n");
        uint8_t op;
        if (!decode_byte(&inbound, &op)) {
            goto Done;
        }
        switch (op) {
        case REQ_EXIT:
            printf("Runner exiting\n");
            r = 0;
            goto Done;
        case REQ_EXE_FOR_PIDS: {
            printf("Runner gets pids\n");
            uint32_t nelem;
            if (!decode_int(&inbound, &nelem)) {
                goto Done;
            }
            if (!encode_byte(&outbound, op)) {
                goto Done;
            }
            if (!encode_int(&outbound, nelem)) {
                goto Done;
            }
            for ( uint32_t i=0 ; i < nelem ; i++ ) {
                uint32_t pid;
                static char exebuf[PATH_MAX];
                if (!decode_int(&inbound, &pid)) {
                    goto Done;
                }
                if (!get_exe(pid, exebuf)) {
                    *exebuf = 0;
                }
                if (!encode_int(&outbound, pid)) {
                    goto Done;
                }
                if (!encode_string(&outbound, exebuf)) {
                    goto Done;
                }
            }
            if (!send_message(output, &outbound)) {
                goto Done;
            }
            continue;
        }
        default:
            fprintf(stderr, "Unknown message: %d\n", op);
            continue;
        }
    }
Done:
    destroy_inbound(&inbound);
    destroy_outbound(&outbound);
    return r;
}

int get_exe(uint32_t pid, char buf[PATH_MAX]) {
    static char path[128];
    snprintf(path, sizeof(path), "/proc/%d/exe", pid);
    ssize_t n;
    if ((n = readlink(path, buf, PATH_MAX)) == -1) {
        return 1;
    }
    if (n >= PATH_MAX) {
        n = PATH_MAX-1;
    }
    buf[n] = 0;
    return 0;
}
