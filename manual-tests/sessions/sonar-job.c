/* The job of this program is to be the parent process for a bunch of worker processes, which we run
 * serially as that provides the best signal. */

#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

/* TODO: This could be a parameter */
#define ITERATIONS 5

int main(int argc, char **argv) {
    /* If this is not the top of a process group, make it one */
    if (getpid() != getpgid(0)) {
        if (setpgid(0, 0) == -1) {
            perror("Trying to create a new process group");
            exit(1);
        }
    }
    assert(getpid() == getpgid(0));

    /* If subjobs are wanted, create them */
    pid_t subjob = 0;
    if (argc > 1) {
        int n = atoi(argv[1]);
        if (n > 0) {
            switch (subjob = fork()) {
              case -1:
                perror("Trying to fork a subjob");
                exit(1);
              case 0: {
                  char buf[20];
                  sprintf(buf, "%d", n-1);
                  execl("sonar-job","sonar-job",buf,NULL);
                  perror("Trying to exec a subjob");
                  exit(1);
              }
            }
        }
    }

    /* Do the work */
    for ( int i=0 ; i < ITERATIONS ; i++ ) {
        pid_t worker;
        switch (worker = fork()) {
          case -1:
            perror("Trying to fork a new process for sonar-worker");
            exit(1);
          case 0:
            execl("sonar-worker", "sonar-worker", NULL);
            perror("Trying to exec sonar-worker");
            exit(1);
          default:
            if (waitpid(worker, NULL, 0) == -1) {
                perror("Waiting for sonar-worker");
                exit(1);
            }
        }
    }

    /* Wait for the subjob to be done */
    if (subjob > 0) {
        waitpid(subjob, NULL, 0);
    }

    /* Wait a bit, so that information from the terminated job can be accounted to the current
       process. */
    printf("Waiting 10s in sonar-job for things to settle...\n");
    sleep(10);
}
