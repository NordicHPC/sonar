/* Run this with SONARTEST_ROLLUP=1 and --rollup --batchless.

   This will fork off a 9 child processes (rollupchild x 5 and rollupchild2 x 4) that are the
   same except for the name, all of which will wait 10s.  Sonar, run meanwhile, should rollup
   the children with the same name only.  Since the rollup field represents n-1 processes,
   the count for rollupchild should be 4 and for rollupchild2 should be 3.

   (This code is -std=c89, try to keep it that way.)
*/

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/wait.h>

#define TYPE1 5
#define TYPE2 4

int main(int argc, char** argv) {
    int i;
    for ( i=0 ; i < TYPE1+TYPE2 ; i++ ) {
        switch (fork()) {
          case -1:
            perror("Forking child");
            exit(1);
          case 0:
            if (i < TYPE1) {
                execl("rollupchild", "rollupchild", NULL);
                fprintf(stderr, "Failed to exec child\n");
                exit(1);
            } else {
                execl("rollupchild2", "rollupchild2", NULL);
                fprintf(stderr, "Failed to exec child 2\n");
                exit(1);
            }
        }
    }
    for ( i=0 ; i < TYPE1+TYPE2 ; i++ ) {
        wait(NULL);
    }
    return 0;
}
