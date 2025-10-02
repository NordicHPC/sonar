/* Run this with SONARTEST_ROLLUP=1 and --rollup.
   If you grep the sonar output for ',cmd=rollup,' there should be 23 lines.
   Of those, there should be eight that have ',rolledup=1' and none that have any other rollup
   fields.

   (This code is -std=c89, try to keep it that way.)
*/

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char** argv) {
    long depth;
    pid_t c1, c2;
    if (argc < 2) {
        fprintf(stderr, "Usage: %s depth\n", argv[0]);
        exit(1);
    }
    errno = 0;
    depth = strtol(argv[1], NULL, 10);
    if (errno != 0 || depth < 0 || depth > 10) {
        fprintf(stderr, "Bad depth\n");
        exit(1);
    }
again:
    /* in Parent */
    c1 = fork();
    if (c1 == -1) {
        perror("Forking child");
        exit(1);
    }
    if (c1 > 0) {
        /* in Parent */
        c2 = fork();
        if (c2 == -1) {
            perror("Forking child");
            exit(1);
        }
        if (c2 > 0) {
            /* in Parent */
            /* printf("Waiting %d\n", getpid()); */
            wait(NULL);
            wait(NULL);
        } else {
            /* in C2 */
            if (depth-- > 0) {
                goto again;
            }
            /* printf("Sleeping %d\n", getpid()); */
            sleep(10);
        }
    } else {
        /* in C1 */
        if (depth-- > 0) {
            goto again;
        }
        /* printf("Sleeping %d\n", getpid()); */
        sleep(10);
    }
    return 0;
}
