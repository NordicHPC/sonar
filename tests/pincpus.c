#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char** argv) {
    int num_children = 0, time_to_wait = 0;
    if (argc == 4) {
        num_children = atoi(argv[2]);
        time_to_wait = atoi(argv[3]);
    }
    if (argc != 4 || num_children <= 0 || time_to_wait <= 0) {
        fprintf(stderr, "Usage: pincpus subprogram-path num-children time-to-wait\n");
        exit(2);
    }
    int i;
    for (i = 0; i < num_children; i++) {
        switch (fork()) {
        case -1:
            perror("fork");
            exit(1);
        case 0:
            execl(argv[1], argv[1], argv[3], NULL);
            perror("exec");
            exit(1);
        }
    }
    for (i = 0; i < num_children; i++) {
        wait(NULL);
    }
    return 0;
}
