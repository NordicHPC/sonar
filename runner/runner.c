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
*/

#define _GNU_SOURCE
#include <fcntl.h>
#include <inttypes.h>
#include <string.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <signal.h>

#include "proto.h"

void msg(const char* s) {
    write(2, s, strlen(s));
}

void sonar(const char* path, const char* config, const char* user, const char* group, int input, int output) {
    printf("Sonar: %d %d\n", input, output);
    char ins[20], outs[20];
    sprintf(ins, "%d", input);
    sprintf(outs, "%d", output);
    int r = execl(path, path, "-i", ins, "-o", outs, "daemon", config, (char*)NULL);
    perror("exec");
}

int server(int input, int output) {
    for (;;) {
        uint8_t op;
        errno = 0;
        int n = read(input, (char*)&op, 1);
        if (n == 0) {
            msg("Unexpected EOF\n");
            return 1;
        }
        if (n < 0) {
            if (errno == EAGAIN) {
                continue;
            }
            perror("server read");
            return 1;
        }
        printf("Server: got msg %d\n", op);
        switch (op) {
        case REQ_EXIT:
            return 0;
        case REQ_EXE_FOR_PID:
            msg("Server: sending reply\n");
            /* TODO: Here the message would have a PID payload */
            /* TODO: This is a little scary because we can hang here if the child is not reading */
            sr_len_t len;
            encode_length(5, len);
            if (write(output, len, 2) != 2) {
                perror("server write");
            }
            if (write(output, "hello", 5) != 5) {
                perror("server write");
            }
            break;
        default:
            msg("Unknown message\n");
            continue;
        }
    }
}

int main(int argc, char** argv) {
    if (argc != 5) {
        msg("Wrong number of arguments\n");
        return 1;
    }
#if 0
    if (getuid() != 0) {
        /* ERROR */
        return 1;
    }
#endif

    /* I don't think we need O_DIRECT here so just pipe() is enough? */

    int down[2];
    if (pipe2(down, 0) == -1) {
        perror("pipe2");
        return 1;
    }
    int up[2];
    if (pipe2(up, 0) == -1) {
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
