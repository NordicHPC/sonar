/* Busy-wait on one core for a time.  Takes an optional argument of the number of seconds to run. */

/* Probably helpful to compile this with -O0 or -O1 to avoid the compiler "simplifying" fib() */

#include <inttypes.h>
#include <stdio.h>
#include <time.h>

static uintptr_t fib(uintptr_t n);

int main(int argc, char** argv) {
    unsigned time_to_run = 5;
    if (argc > 1) {
        sscanf(argv[1], "%u", &time_to_run);
    }
    time_t end_time = time(NULL) + time_to_run;
    uintptr_t sum = 0;
    while (time(NULL) < end_time) {
        /* On early-2020s hardware, fib(42) compiled with -O1 takes about 1s */
        sum += fib(42);
    }
    printf("%llu\n", (unsigned long long)sum);
    return 0;
}

static uintptr_t fib(uintptr_t n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}
