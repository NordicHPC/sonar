#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

int main(int argc, char **argv) {
    /* When the shell starts this process it may make it a process group leader, otherwise, become
       one. */
    if (getpid() != getpgid(0)) {
        if (setsid() == (pid_t)-1) {
            perror("Trying to become session leader");
            exit(1);
        }
    }

    assert(getpid() == getpgid(0));

    /* Fork off the job root below us. */
    pid_t child = fork();
    if (child == (pid_t)-1) {
        perror("Trying to fork a new process for sonar-job-root");
        exit(1);
    }
    if (child == 0) {
        execl("sonar-job-root", "sonar-job-root", NULL);
        perror("Trying to exec sonar-job-root");
        exit(1);
    }
    if (wait(NULL) == -1) {
        perror("Waiting for sonar-job-root");
        exit(1);
    }
}
