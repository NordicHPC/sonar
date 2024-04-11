/* The job of this program is to be the parent process for a bunch of worker processes, which we run
 * serially as that provides the best signal. */

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

/* TODO: This could be a parameter */
#define ITERATIONS 5

int main(int argc, char **argv) {
    for ( int i=0 ; i < ITERATIONS ; i++ ) {
        pid_t child = fork();
        if (child == (pid_t)-1) {
            perror("Trying to fork a new process for sonar-worker");
            exit(1);
        }
        if (child == 0) {
            execl("sonar-worker", "sonar-worker", NULL);
            perror("Trying to exec sonar-worker");
            exit(1);
        }
        if (wait(NULL) == -1) {
            perror("Waiting for sonar-worker");
            exit(1);
        }
    }
}
