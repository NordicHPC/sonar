#include <dlfcn.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>

#include "sonar-amd.h"

static int load_amdml() {
    static void* lib;

    if (lib != NULL) {
        return 0;
    }

    /* This is the location of the library on all the ml4. It is also where AMD says it should be. */
    lib = dlopen("/opt/rocm/lib/libamd_smi.so", RTLD_NOW);
    if (lib == NULL) {
        printf("Could not load library");
        return -1;
    }

    return 0;
}

int amdml_device_get_count(uint32_t* count) {
    load_amdml();
    *count = 0;
    return 0;
}

