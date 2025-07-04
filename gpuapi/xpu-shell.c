/* Test code.  See Makefile for how to build. */

/* Usage:
    -info  print card info (default)
    -state print card state
    -proc  print processes
*/

#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>

#include "sonar-xpu.h"

void panic(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    fprintf(stderr, "PANIC: ");
    vfprintf(stderr, fmt, args);
    fprintf(stderr, "\n");
    va_end(args);
    exit(1);
}

enum op_t {
    INFO,
    STATE,
    PROC
};

int main(int argc, char** argv) {
    enum op_t mode = INFO;
    if (argc < 2 || strcmp(argv[1], "-h") == 0) {
        printf("Usage: xpu-shell [options]\n");
        printf("Options:\n");
        printf(" -info - print card info\n");
        printf(" -state - print card state\n");
        printf(" -proc - print card process info\n");
        exit(2);
    }

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
    int r = xpu_device_get_count(&count);
    if (r == -1) {
        panic("Failed get_count");
    }

    printf("\n%u devices\n", count);

    switch (mode) {
      case INFO: {
          struct xpu_card_info_t info;
          for ( uint32_t dev = 0 ; dev < count ; dev++ ) {
              memset(&info, 0, sizeof(info));
              int r = xpu_device_get_card_info(dev, &info);
              if (r == -1) {
                  panic("Failed to get card info for %u", dev);
              }
              printf("\nDEVICE %u\n", dev);
              printf("  bus %s\n", info.bus_addr);
              printf("  model %s\n", info.model);
              printf("  driver %s\n", info.driver);
              printf("  firmware %s\n", info.firmware);
              printf("  uuid %s\n", info.uuid);
              printf("  memory %llu\n", (unsigned long long)info.totalmem);
              printf("  max_ce_clk %u\n", info.max_ce_clock);
              printf("  max_plim %u\n", info.max_power_limit);
          }
          break;
      }
      case STATE: {
          struct xpu_card_state_t info;
          for ( uint32_t dev = 0 ; dev < count ; dev++ ) {
              memset(&info, 0, sizeof(info));
              int r = xpu_device_get_card_state(dev, &info);
              if (r == -1) {
                  panic("Failed to get card state for %u", dev);
              }
              printf("\nDEVICE %u\n", dev);
              printf("  used %llu\n", (unsigned long long)info.mem_used);
              printf("  gpu%% %g\n", info.gpu_util);
              printf("  mem%% %g\n", info.mem_util);
              printf("  temp %u\n", info.temp);
              printf("  power %u\n", info.power);
              printf("  ce_clk %u\n", info.ce_clock);
          }
          break;
      }
      case PROC: {
          for ( uint32_t dev = 0 ; dev < count ; dev++ ) {
              struct xpu_gpu_process_t proc;
              uint32_t pcount;
              int r = xpu_device_probe_processes(dev, &pcount);
              if (r == -1) {
                  panic("Failed to get processes for %u\n", dev);
              }
              printf("\nDEVICE %u\n", dev);
              for ( uint32_t p = 0 ; p < pcount ; p++ ) {
                  memset(&proc, 0, sizeof(proc));
                  r = xpu_get_process(p, &proc);
                  if (r == -1) {
                      panic("Failed to get process for %u: %u\n", dev, p);
                  }
                  printf(" PROCESS %u\n", p);
                  printf("  pid %u\n", proc.pid);
                  printf("  mem %u\n", proc.mem_util);
                  printf("  gpu %u\n", proc.gpu_util);
                  printf("  sz %llu\n", (unsigned long long)proc.mem_size);
              }
              xpu_free_processes();
          }
          break;
      }
    }
}
