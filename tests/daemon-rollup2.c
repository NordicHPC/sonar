/* This will first fork off 6 child processes (drchild0 x 3 and drchild1 x 3), all of which will
   wait 10s.  Then it will wait 5s and do the same with drchild2 and drchild3.  (So, total 25s.)
   Then it will wait 5s and repeat.  The point of using the separate names is to rollup into
   separate processes.  The point of the short names is that long names ("daemon-rollupchildN") are
   chopped by the OS.

   The sonar daemon should run with a time limit of 60s.

   (This code is -std=c89, try to keep it that way.)
*/

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#define TYPE1 5
#define TYPE2 4

int main(int argc, char** argv) {
    int i, r, k;
    char name[128];
    for (r = 0; r < 2; r++) {
        for (k = 0; k < 2; k++) {
            if (!(r == 0 && k == 0)) {
                sleep(5);
            }
            for (i = 0; i < 6; i++) {
                switch (fork()) {
                case -1:
                    perror("Forking child");
                    exit(1);
                case 0:
                    sprintf(name, "drchild%d", i / 3 + k * 2);
                    fprintf(stderr, "Starting %s\n", name);
                    execl(name, name, NULL);
                    fprintf(stderr, "Failed to exec child %s\n", name);
                    exit(1);
                }
            }
            for (i = 0; i < 6; i++) {
                wait(NULL);
            }
        }
    }
    return 0;
}
