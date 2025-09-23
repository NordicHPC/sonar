/* This creates a busy computation running a number of threads.  The program takes two arguments:
 * the number of threads and the duration of the test in minutes. */

/* Probably helpful to compile this with -O0 or -O1 to avoid the compiler "simplifying" fib() */

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <inttypes.h>

static void* worker(void* arg);
static uintptr_t fib(uintptr_t n);

int main(int argc, char** argv) {
    unsigned num_threads, duration;
    if (argc != 3 ||
        sscanf(argv[1], "%u", &num_threads) != 1 ||
        num_threads < 1 || num_threads > 1000 ||
        sscanf(argv[2], "%u", &duration) != 1 ||
        duration < 1 || duration > 3600)
    {
        fprintf(stderr, "Usage: %s num-threads duration-in-minutes\n", argv[0]);
        exit(1);
    }
    time_t then = time(NULL);
    time_t end_time = then + 60*(time_t)duration;
    pthread_t *handles = calloc(num_threads-1, sizeof(pthread_t));
    if (handles == NULL) {
        fprintf(stderr, "Alloc failure\n");
        exit(1);
    }
    for ( unsigned t = 0 ; t < num_threads-1 ; t++ ) {
        if (pthread_create(&handles[t], NULL, worker, &end_time) != 0) {
            fprintf(stderr, "Unable to create thread\n");
            exit(1);
        }
    }
    worker(&end_time);
    uintptr_t sum = 0;
    for ( unsigned t = 0 ; t < num_threads-1 ; t++ ) {
        void* x;
        pthread_join(handles[t], &x);
        sum += (uintptr_t)x;
    }
    printf("Time: %lds\n", (time(NULL)-then));
    printf("Result %lld\n", (long long)sum);
}

static void* worker(void* arg) {
    time_t end_time = *(time_t*)arg;
    uintptr_t sum = 0;
    for (;;) {
        if (time(NULL) > end_time) {
            return (void*)sum;
        }
        /* On ML1, fib(47) compiled with -O1 takes about 20s */
        sum += fib(47);
    }
}

static uintptr_t fib(uintptr_t n) {
    if (n < 2) {
        return n;
    }
    return fib(n-1) + fib(n-2);
}
