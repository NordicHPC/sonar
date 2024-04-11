#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

/* This could be a parameter */
#define ITERATIONS 5

/* TODO: Whether we run things in parallel (multicore is a thing) could be a parameter,
 * complicates the wait logic slightly.
 */

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
