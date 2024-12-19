/* Remember to `module load CUDA/11.1.1-GCC-10.2.0` or similar for nvml.h.

   On the UiO ML nodes, header files are here:
     /storage/software/CUDA/11.3.1/targets/x86_64-linux/include/nvml.h
     /storage/software/CUDAcore/11.1.1/targets/x86_64-linux/include/nvml.h
*/

#include <stddef.h>
#include <string.h>
#include <inttypes.h>
#include <dlfcn.h>
#include <nvml.h>

#include "sonar-nvidia.h"

static void* lib;
static nvmlReturn_t (*xnvmlInit)();
static nvmlReturn_t (*xnvmlDeviceGetCount_v2)(unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetHandleByIndex_v2)(int index, nvmlDevice_t* dev);
static nvmlReturn_t (*xnvmlDeviceGetArchitecture)(nvmlDevice_t, nvmlDeviceArchitecture_t*);
static nvmlReturn_t (*xnvmlDeviceGetMemoryInfo)(nvmlDevice_t, nvmlMemory_t*);
static nvmlReturn_t (*xnvmlDeviceGetFanSpeed)(nvmlDevice_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetName)(nvmlDevice_t,char*,unsigned);
static nvmlReturn_t (*xnvmlDeviceGetUUID)(nvmlDevice_t,char*,unsigned);
static nvmlReturn_t (*xnvmlDeviceGetPowerManagementLimitConstraints)(nvmlDevice_t,unsigned*,unsigned*);
static nvmlReturn_t (*xnvmlSystemGetDriverVersion)(char*,unsigned);

static int load_nvml() {
    if (lib == NULL) {
        lib = dlopen("/usr/lib64/libnvidia-ml.so", RTLD_NOW);
        if (lib == NULL) {
            return -1;
        }
    }

    /* You'll be tempted to try some magic here with # and ## but it won't work because sometimes
       nvml.h introduces #defines of some of the names we want to use. */

#define DLSYM(var, str) if ((var = dlsym(lib, str)) == NULL) { return -1; }

    DLSYM(xnvmlInit, "nvmlInit");
    DLSYM(xnvmlDeviceGetCount_v2, "nvmlDeviceGetCount_v2");
    DLSYM(xnvmlDeviceGetHandleByIndex_v2, "nvmlDeviceGetHandleByIndex_v2");
    DLSYM(xnvmlDeviceGetArchitecture, "nvmlDeviceGetArchitecture");
    DLSYM(xnvmlDeviceGetFanSpeed, "nvmlDeviceGetFanSpeed");
    DLSYM(xnvmlDeviceGetMemoryInfo, "nvmlDeviceGetMemoryInfo");
    DLSYM(xnvmlDeviceGetName, "nvmlDeviceGetName");
    DLSYM(xnvmlDeviceGetUUID, "nvmlDeviceGetUUID");
    DLSYM(xnvmlSystemGetDriverVersion, "nvmlSystemGetDriverVersion");
    DLSYM(xnvmlDeviceGetPowerManagementLimitConstraints, "nvmlDeviceGetPowerManagementLimitConstraints");

    return 0;
}

static void unload_nvml() {
    dlclose(lib);
    lib = NULL;
}

int nvml_open() {
    if (load_nvml() != 0) {
        return -1;
    }
    if (xnvmlInit() != 0) {
        return -1;
    }
    return 0;
}

int nvml_close() {
    if (lib == NULL) {
        return -1;
    }
    unload_nvml();
    return 0;
}

int nvml_device_get_count(uint32_t* count) {
    if (lib == NULL) {
        return -1;
    }
    unsigned ndev;
    if (xnvmlDeviceGetCount_v2(&ndev) != 0) {
        return -1;
    }
    *count = ndev;
    return 0;
}

int nvml_device_get_card_info(uint32_t device, struct nvml_card_info* infobuf) {
    // FIXME:
    // - bus_addr
    // - firmware
    // - power_limit_watt
    // - max_ce_clock
    // - max_mem_clock

    if (lib == NULL) {
        return -1;
    }
    nvmlDevice_t dev;
    if (xnvmlDeviceGetHandleByIndex_v2(device, &dev) != 0) {
        return -1;
    }
    memset(infobuf, 0, sizeof(*infobuf));

    xnvmlDeviceGetName(dev, infobuf->model, sizeof(infobuf->model));
    xnvmlDeviceGetUUID(dev, infobuf->uuid, sizeof(infobuf->uuid));
    xnvmlSystemGetDriverVersion(infobuf->driver, sizeof(infobuf->driver));
    xnvmlDeviceGetPowerManagementLimitConstraints(dev, &infobuf->min_power_limit, &infobuf->max_power_limit);

    nvmlDeviceArchitecture_t n_arch;
    if (xnvmlDeviceGetArchitecture(dev, &n_arch) == 0) {
        const char* archname;
        /* The architecture numbers are taken from the CUDA 12.3.0 nvml.h.  We could #ifdef and
           switch on the appropriate #defines here but that locks us in to compiling with the newest
           header files, and that's not desirable, hence use the literal numbers. */
        switch (n_arch) {
          case 2:
            archname = "Kepler";
            break;
          case 3:
            archname = "Maxwell";
            break;
          case 4:
            archname = "Pascal";
            break;
          case 5:
            archname = "Volta";
            break;
          case 6:
            archname = "Turing";
            break;
          case 7:
            archname = "Ampere";
            break;
          case 8:
            archname = "Ada";
            break;
          case 9:
            archname = "Hopper";
            break;
          case 10:              /* I'm guessing */
            archname = "Blackwell";
            break;
          default:
            archname = "(unknown)";
            break;
        }
        strcpy(infobuf->architecture, archname);
    }

    nvmlMemory_t mem;
    if (xnvmlDeviceGetMemoryInfo(dev, &mem) == 0) {
        infobuf->totalmem = mem.total;
    }

    return 0;
}

int nvml_device_get_card_state(uint32_t device, struct nvml_card_state* infobuf) {
    // FIXME:
    // more fields

    if (lib == NULL) {
        return -1;
    }
    nvmlDevice_t dev;
    if (xnvmlDeviceGetHandleByIndex_v2(device, &dev) != 0) {
        return -1;
    }
    memset(infobuf, 0, sizeof(*infobuf));

    xnvmlDeviceGetFanSpeed(dev, &infobuf->fan_speed);
    // etc

    return 0;
}

/* Dynamic library management */

