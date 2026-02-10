/* Static-linkable wrapper for a fake GPU.  See sonar-fakegpu.h and Makefile for more. */

#include <assert.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "sonar-fakegpu.h"
#include "strtcpy.h"

static uint32_t num_gpus = 1;

int fakegpu_device_get_count(uint32_t* count) {
    *count = num_gpus;
    return 0;
}

int fakegpu_device_get_card_info(uint32_t device_index, struct fakegpu_card_info_t* infobuf) {
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    memset(infobuf, 0, sizeof(*infobuf));
    strcpy(infobuf->bus_addr, "0:0:0:fake");
    strcpy(infobuf->model, "fake-model");
    strcpy(infobuf->driver, "fake-driver");
    strcpy(infobuf->firmware, "fake-firmware");
    strcpy(infobuf->uuid, "fake:0");
    infobuf->totalmem = (uint64_t)4 * 1024 * 1024 * 1024;
    infobuf->max_ce_clock = 1000;
    infobuf->max_power_limit = 1000;
    return 0;
}

int fakegpu_device_get_card_state(uint32_t device_index, struct fakegpu_card_state_t* infobuf) {
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    memset(infobuf, 0, sizeof(*infobuf));
    infobuf->gpu_util = 95;
    infobuf->mem_util = 88;
    infobuf->mem_used = (uint64_t)4 * 1024 * 1024 * 1024 * 88 / 100;
    infobuf->temp = 37;
    infobuf->power = 200;
    infobuf->ce_clock = 666;
    return 0;
}

static uint32_t info_count = 1;

int fakegpu_device_probe_processes(uint32_t device_index, uint32_t* count) {
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }
    *count = info_count;
    return 0;
}

int fakegpu_get_process(uint32_t process_index, struct fakegpu_gpu_process_t* infobuf) {
    if (process_index >= info_count) {
        return -1;
    }
    infobuf->pid = 12579;
    infobuf->mem_util = 50;
    infobuf->gpu_util = 90;
    infobuf->mem_size = (uint64_t)2 * 1024 * 1024 * 1024;
    return 0;
}

void fakegpu_free_processes() { }
