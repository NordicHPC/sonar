/* Test code.  See Makefile for how to build. */

/* Usage:
    -info  print card info (default)
    -state print card state
    -proc  print processes
*/

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "sonar-nvidia.h"

void panic(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    fprintf(stderr, "PANIC: ");
    vfprintf(stderr, fmt, args);
    fprintf(stderr, "\n");
    va_end(args);
    exit(1);
}

enum op_t { INFO, STATE, PROC };

int main(int argc, char** argv) {
    enum op_t mode = INFO;
    if (argc >= 2) {
        if (strcmp(argv[1], "-info") == 0) {
            mode = INFO;
        } else if (strcmp(argv[1], "-state") == 0) {
            mode = STATE;
        } else if (strcmp(argv[1], "-proc") == 0) {
            mode = PROC;
        } else {
            panic("Bad argument");
        }
    }

    uint32_t count;
    int r = nvml_device_get_count(&count);
    if (r == -1) {
        panic("Failed get_count");
    }

    printf("\n%u devices\n", count);

    switch (mode) {
    case INFO: {
        struct nvml_card_info info;
        for (uint32_t dev = 0; dev < count; dev++) {
            memset(&info, 0, sizeof(info));
            int r = nvml_device_get_card_info(dev, &info);
            if (r == -1) {
                panic("Failed to get card info for %u", dev);
            }
            printf("\nDEVICE %u\n", dev);
            printf("  bus %s\n", info.bus_addr);
            printf("  model %s\n", info.model);
            printf("  arch %s\n", info.architecture);
            printf("  driver %s\n", info.driver);
            printf("  firmware %s\n", info.firmware);
            printf("  uuid %s\n", info.uuid);
            printf("  memory %llu\n", (unsigned long long)info.totalmem);
            printf("  plim %u\n", info.power_limit);
            printf("  min_plim %u\n", info.min_power_limit);
            printf("  max_plim %u\n", info.max_power_limit);
            printf("  max_ce_clk %u\n", info.max_ce_clock);
            printf("  max_mem_clk %u\n", info.max_mem_clock);
        }
        break;
    }
    case STATE: {
        struct nvml_card_state info;
        for (uint32_t dev = 0; dev < count; dev++) {
            memset(&info, 0, sizeof(info));
            int r = nvml_device_get_card_state(dev, &info);
            if (r == -1) {
                panic("Failed to get card state for %u", dev);
            }
            printf("\nDEVICE %u\n", dev);
            printf("  fan%% %u\n", info.fan_speed);
            printf("  mode %d\n", info.compute_mode);
            printf("  state %d\n", info.perf_state);
            printf("  reserved %llu\n", (unsigned long long)info.mem_reserved);
            printf("  used %llu\n", (unsigned long long)info.mem_used);
            printf("  gpu%% %g\n", info.gpu_util);
            printf("  mem%% %g\n", info.mem_util);
            printf("  temp %u\n", info.temp);
            printf("  power %u\n", info.power);
            printf("  plim %u\n", info.power_limit);
            printf("  ce_clk %u\n", info.ce_clock);
            printf("  mem_clk %u\n", info.mem_clock);
        }
        break;
    }
    case PROC: {
        for (uint32_t dev = 0; dev < count; dev++) {
            struct nvml_gpu_process proc;
            uint32_t pcount;
            int r = nvml_device_probe_processes(dev, &pcount);
            if (r == -1) {
                panic("Failed to get processes for %u\n", dev);
            }
            printf("\nDEVICE %u\n", dev);
            for (uint32_t p = 0; p < pcount; p++) {
                memset(&proc, 0, sizeof(proc));
                r = nvml_get_process(p, &proc);
                if (r == -1) {
                    panic("Failed to get process for %u: %u\n", dev, p);
                }
                printf(" PROCESS %u\n", p);
                printf("  pid %u\n", proc.pid);
                printf("  mem %u\n", proc.mem_util);
                printf("  gpu %u\n", proc.gpu_util);
                printf("  sz %llu\n", (unsigned long long)proc.mem_size);
            }
            nvml_free_processes();
        }
        break;
    }
    }
}
