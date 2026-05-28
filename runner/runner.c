/* This is meant to run as root.  It forks off a non-privileged Sonar and sets itself up as a
   partner that will respond to very limited requests for information that is only available to
   root.  Communication is over a pipe: Sonar may send questions and this program will respond
   with an answer.

   Usage (as root):
     sonar-daemon-runner path-to-sonar path-to-daemon-config-file user group

   This will create a bidirectional pipe, fork & drop provileges to user/group & run sonar
   with the arguments (and passing the pipe info on the command line), then wait for commands.
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

/* An outgoing String is a (say) uint16_t length followed by raw bytes, with no terminator. */
/* Batching would be nice but makes the protocol a little harder? */

/* Arguments:
   -user   sonar-user-name
   -group  sonar-group-name
   -sonar  path-to-sonar-executable
   -config path-to-sonar-config-file

   Chief problem:

   We're going to lower privileges and then run `sonar daemon config-file` but we need to coordinate
   the name of the communication channel and we ideally want this channel to be invisible to anyone
   outside the two processes.

   A pipe would be best but then the Sonar subprocess must be able to know the proper FDs and we
   need to guarantee that those FDs are not being used by the Rust runtime.  I'm guessing this will
   be OK and that the FDs can be communicated by env vars, in the worst case.
*/

void msg(const char* s) {
    write(2, s, strlen(s));
}

void sonar(const char* path, const char* config, const char* user, const char* group, int input, int output) {
    printf("Sonar: %d %d\n", input, output);
    char ins[20], outs[20];
    sprintf(ins, "%d", input);
    sprintf(outs, "%d", output);
    /* TODO: Could close some things that are not needed */
    int r = execl(path, path, "-pin", ins, "-pout", outs, "daemon", config, (char*)NULL);
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
            if (write(output, "\x05\x00hello", 7) != 7) {
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

    /* I don't think we need O_DIRECT here */

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
        return server(up[0], down[1]);
    default:
        sonar(argv[1], argv[2], argv[3], argv[4], down[0], up[1]);
        return 1;
    }
}
