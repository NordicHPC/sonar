/* This stands in for sonar.  The arguments -pin and -pout carry the descriptors to use for the
 * pipe.  Sonar will want to batch messages and will want to get error returns for arguments that
 * can't be handled (eg processes that have exited).  There needs to be a timeout on reading from
 * the service, not more than a few seconds probably.  If the service times out it should be assumed
 * to be suspec, and if twice it should be assumed to be dead, and an error should be propagated
 * accordingly.
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
        if (strcmp(*argv, "-pin") == 0) {
            argv++;
            if (*argv == 0) {
                msg("Bad args\n");
                return 1;
            }
            sscanf(*argv, "%d", &input);
            argv++;
            continue;
        }
        if (strcmp(*argv, "-pout") == 0) {
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
        uint8_t op = REQ_EXE_FOR_PID;
        /* TODO: Need to send a payload here */
        if (write(output, &op, 1) != 1) {
            perror("client write");
            return 1;
        }
        uint8_t len[2];
        if (read(input, &len, sizeof(len)) != 2) {
            perror("client read");
            return 1;
        }
        size_t slen = ((size_t)len[1] << 8) + (size_t)len[0];
        printf("will read %d bytes\n", slen);
        char *buf = malloc(slen);
        if (buf == NULL) {
            perror("malloc");
            return 1;
        }
        if (read(input, buf, slen) != slen) {
            perror("client read 2");
            return 1;
        }
        buf[slen] = 0;
        printf("%s\n", buf);
        free(buf);
    }
}
