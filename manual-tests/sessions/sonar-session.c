/* The job of this program is to be the a session leader above the job root. */

#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

int main(int argc, char **argv) {
    /* When the shell starts this process it may make it a process group leader, otherwise, become one. */
    if (getpid() != getpgid(0)) {
        if (setpgid(0, 0) == -1) {
            perror("Trying to become process group leader");
            exit(1);
        }
    }

    assert(getpid() == getpgid(0));

    /* Fork off the job root below us. */
    pid_t child = fork();
    if (child == (pid_t)-1) {
        perror("Trying to fork a new process for sonar-job");
        exit(1);
    }
    if (child == 0) {
        execl("sonar-job", "sonar-job", NULL);
        perror("Trying to exec sonar-job");
        exit(1);
    }
    if (wait(NULL) == -1) {
        perror("Waiting for sonar-job");
        exit(1);
    }

    /* Wait a bit, so that information from the terminated job can be accounted to the current
       process. */
    printf("Waiting 10s in sonar-session for things to settle...\n");
    sleep(10);
}
