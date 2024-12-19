/* Static-linkable wrapper around the NVIDIA NVML dynamic library with some abstractions
   for our needs.  See sonar-nvidia.h and Makefile for more.

   Note you may need to `module load CUDA/11.1.1-GCC-10.2.0` or similar for access to nvml.h, it may
   not be loaded by default.

   On the UiO ML nodes, nvml.h is here (etc for other versions):
     /storage/software/CUDA/11.3.1/targets/x86_64-linux/include/nvml.h
     /storage/software/CUDAcore/11.1.1/targets/x86_64-linux/include/nvml.h
*/

#include <assert.h>
#include <dlfcn.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h> /* snprintf, we can fix this if we don't want the baggage */
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <nvml.h>

#include "sonar-nvidia.h"

static nvmlReturn_t (*xnvmlDeviceGetClockInfo)(nvmlDevice_t,nvmlClockType_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetComputeMode)(nvmlDevice_t,nvmlComputeMode_t*);
static nvmlReturn_t (*xnvmlDeviceGetComputeRunningProcesses_v3)(
    nvmlDevice_t,unsigned*,nvmlProcessInfo_t*);
static nvmlReturn_t (*xnvmlDeviceGetCount_v2)(unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetHandleByIndex_v2)(int index, nvmlDevice_t* dev);
static nvmlReturn_t (*xnvmlDeviceGetArchitecture)(nvmlDevice_t, nvmlDeviceArchitecture_t*);
static nvmlReturn_t (*xnvmlDeviceGetFanSpeed)(nvmlDevice_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetMemoryInfo)(nvmlDevice_t, nvmlMemory_t*);
static nvmlReturn_t (*xnvmlDeviceGetMaxClockInfo)(nvmlDevice_t,nvmlClockType_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetName)(nvmlDevice_t,char*,unsigned);
static nvmlReturn_t (*xnvmlDeviceGetPciInfo_v3)(nvmlDevice_t,nvmlPciInfo_t*);
static nvmlReturn_t (*xnvmlDeviceGetPerformanceState)(nvmlDevice_t,nvmlPstates_t*);
static nvmlReturn_t (*xnvmlDeviceGetPowerManagementLimitConstraints)(
    nvmlDevice_t,unsigned*,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetPowerManagementLimit)(nvmlDevice_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetPowerUsage)(nvmlDevice_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetProcessUtilization)(
    nvmlDevice_t,nvmlProcessUtilizationSample_t*,unsigned*,unsigned long long);
static nvmlReturn_t (*xnvmlDeviceGetTemperature)(nvmlDevice_t,nvmlTemperatureSensors_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetUUID)(nvmlDevice_t,char*,unsigned);
static nvmlReturn_t (*xnvmlDeviceGetUtilizationRates)(nvmlDevice_t,nvmlUtilization_t*);
static nvmlReturn_t (*xnvmlInit)();
static nvmlReturn_t (*xnvmlSystemGetDriverVersion)(char*,unsigned);
static nvmlReturn_t (*xnvmlSystemGetCudaDriverVersion)(int*);

static int load_nvml() {
    static void* lib;

    if (lib != NULL) {
        return 0;
    }

    /* This is the location of the library on all the UiO ML nodes and on the Fox GPU nodes. */
    /* The Web also seems to think this is the right spot. */
    /* TBD is Saga, Betzy GPU nodes. */
    lib = dlopen("/usr/lib64/libnvidia-ml.so", RTLD_NOW);
    if (lib == NULL) {
        return -1;
    }

    /* You'll be tempted to try some magic here with # and ## but it won't work because sometimes
       nvml.h introduces #defines of some of the names we want to use. */

#define DLSYM(var, str) \
    if ((var = dlsym(lib, str)) == NULL) {      \
        /* puts(str); */                        \
        lib = NULL;                             \
        return -1;                              \
    }

    DLSYM(xnvmlDeviceGetClockInfo, "nvmlDeviceGetClockInfo");
    DLSYM(xnvmlDeviceGetComputeMode, "nvmlDeviceGetComputeMode");
    DLSYM(xnvmlDeviceGetComputeRunningProcesses_v3, "nvmlDeviceGetComputeRunningProcesses_v3");
    DLSYM(xnvmlDeviceGetCount_v2, "nvmlDeviceGetCount_v2");
    DLSYM(xnvmlDeviceGetHandleByIndex_v2, "nvmlDeviceGetHandleByIndex_v2");
    DLSYM(xnvmlDeviceGetArchitecture, "nvmlDeviceGetArchitecture");
    DLSYM(xnvmlDeviceGetFanSpeed, "nvmlDeviceGetFanSpeed");
    DLSYM(xnvmlDeviceGetMemoryInfo, "nvmlDeviceGetMemoryInfo");
    DLSYM(xnvmlDeviceGetMaxClockInfo, "nvmlDeviceGetMaxClockInfo");
    DLSYM(xnvmlDeviceGetName, "nvmlDeviceGetName");
    DLSYM(xnvmlDeviceGetPciInfo_v3, "nvmlDeviceGetPciInfo_v3");
    DLSYM(xnvmlDeviceGetPerformanceState, "nvmlDeviceGetPerformanceState");
    DLSYM(xnvmlDeviceGetPowerManagementLimitConstraints,
          "nvmlDeviceGetPowerManagementLimitConstraints");
    DLSYM(xnvmlDeviceGetPowerManagementLimit, "nvmlDeviceGetPowerManagementLimit");
    DLSYM(xnvmlDeviceGetPowerUsage, "nvmlDeviceGetPowerUsage");
    DLSYM(xnvmlDeviceGetProcessUtilization, "nvmlDeviceGetProcessUtilization");
    DLSYM(xnvmlDeviceGetTemperature, "nvmlDeviceGetTemperature");
    DLSYM(xnvmlDeviceGetUUID, "nvmlDeviceGetUUID");
    DLSYM(xnvmlDeviceGetUtilizationRates, "nvmlDeviceGetUtilizationRates");
    DLSYM(xnvmlInit, "nvmlInit");
    DLSYM(xnvmlSystemGetDriverVersion, "nvmlSystemGetDriverVersion");
    DLSYM(xnvmlSystemGetCudaDriverVersion, "nvmlSystemGetCudaDriverVersion");

    if (xnvmlInit() != 0) {
        lib = NULL;
        return -1;
    }

    return 0;
}

int nvml_device_get_count(uint32_t* count) {
    if (load_nvml() == -1) {
        return -1;
    }
    unsigned ndev;
    if (xnvmlDeviceGetCount_v2(&ndev) != 0) {
        return -1;
    }
    *count = ndev;
    return 0;
}

/* The architecture numbers are taken from the CUDA 12.3.0 nvml.h, except Blackwell is a guess. */
static const char* const arch_names[] = {
    "(unknown)",
    "(unknown)",
    "Kepler",
    "Maxwell",
    "Pascal",
    "Volta",
    "Turing",
    "Ampere",
    "Ada",
    "Hopper",
    "Blackwell",
};

int nvml_device_get_card_info(uint32_t device, struct nvml_card_info* infobuf) {
    if (load_nvml() == -1) {
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
    xnvmlDeviceGetPowerManagementLimitConstraints(
        dev, &infobuf->min_power_limit, &infobuf->max_power_limit);

    int cuda;
    if (xnvmlSystemGetCudaDriverVersion(&cuda) == 0) {
        snprintf(infobuf->firmware, sizeof(infobuf->firmware), "%d.%d",
                 NVML_CUDA_DRIVER_VERSION_MAJOR(cuda),
                 NVML_CUDA_DRIVER_VERSION_MINOR(cuda));
    }

    nvmlDeviceArchitecture_t n_arch;
    if (xnvmlDeviceGetArchitecture(dev, &n_arch) == 0) {
        const char* archname = "(unknown)";
        if (n_arch < sizeof(arch_names)/sizeof(arch_names[0])) {
            archname = arch_names[n_arch];
        }
        strcpy(infobuf->architecture, archname);
    }

    nvmlMemory_t mem;
    if (xnvmlDeviceGetMemoryInfo(dev, &mem) == 0) {
        infobuf->totalmem = mem.total;
    }

    unsigned power_limit;
    if (xnvmlDeviceGetPowerManagementLimit(dev, &power_limit) == 0) {
        infobuf->power_limit = power_limit;
    }

    unsigned clock;
    if (xnvmlDeviceGetMaxClockInfo(dev, NVML_CLOCK_SM, &clock) == 0) {
        infobuf->max_ce_clock = clock;
    }
    if (xnvmlDeviceGetMaxClockInfo(dev, NVML_CLOCK_MEM, &clock) == 0) {
        infobuf->max_mem_clock = clock;
    }

    nvmlPciInfo_t pci;
    if (xnvmlDeviceGetPciInfo_v3(dev, &pci) == 0) {
        strncpy(infobuf->bus_addr, pci.busId, sizeof(infobuf->bus_addr));
        infobuf->bus_addr[sizeof(infobuf->bus_addr)-1] = 0;
    }

    return 0;
}

int nvml_device_get_card_state(uint32_t device, struct nvml_card_state* infobuf) {
    if (load_nvml() == -1) {
        return -1;
    }
    nvmlDevice_t dev;
    if (xnvmlDeviceGetHandleByIndex_v2(device, &dev) != 0) {
        return -1;
    }
    memset(infobuf, 0, sizeof(*infobuf));

    xnvmlDeviceGetFanSpeed(dev, &infobuf->fan_speed);

    nvmlMemory_t mem;
    if (xnvmlDeviceGetMemoryInfo(dev, &mem) == 0) {
        infobuf->mem_reserved = mem.total - (mem.free + mem.used);
        infobuf->mem_used = mem.used;
    }

    unsigned power_limit;
    if (xnvmlDeviceGetPowerManagementLimit(dev, &power_limit) == 0) {
        infobuf->power_limit = power_limit;
    }

    unsigned clock;
    if (xnvmlDeviceGetClockInfo(dev, NVML_CLOCK_SM, &clock) == 0) {
        infobuf->ce_clock = clock;
    }
    if (xnvmlDeviceGetClockInfo(dev, NVML_CLOCK_MEM, &clock) == 0) {
        infobuf->mem_clock = clock;
    }

    nvmlComputeMode_t mode;
    if (xnvmlDeviceGetComputeMode(dev, &mode) == 0) {
        switch (mode) {
          case NVML_COMPUTEMODE_DEFAULT:
            infobuf->compute_mode = COMP_MODE_DEFAULT;
            break;
          case NVML_COMPUTEMODE_PROHIBITED:
            infobuf->compute_mode = COMP_MODE_PROHIBITED;
            break;
          case NVML_COMPUTEMODE_EXCLUSIVE_PROCESS:
            infobuf->compute_mode = COMP_MODE_EXCLUSIVE_PROCESS;
            break;
          default:
            infobuf->compute_mode = COMP_MODE_UNKNOWN;
            break;
        }
    }

    nvmlPstates_t pstate;
    if (xnvmlDeviceGetPerformanceState(dev, &pstate) == 0) {
        if (pstate == NVML_PSTATE_UNKNOWN) {
            infobuf->perf_state = PERF_STATE_UNKNOWN;
        } else {
            assert(pstate >= 0);
            infobuf->perf_state = (int)pstate;
        }
    }

    unsigned temp;
    if (xnvmlDeviceGetTemperature(dev, NVML_TEMPERATURE_GPU, &temp) == 0) {
        infobuf->temp = temp;
    }

    unsigned power_draw;
    if (xnvmlDeviceGetPowerUsage(dev, &power_draw) == 0) {
        infobuf->power = power_draw;
    }

    nvmlUtilization_t rates;
    if (xnvmlDeviceGetUtilizationRates(dev, &rates) == 0) {
        infobuf->gpu_util = rates.gpu;
        infobuf->mem_util = rates.memory;
    }

    return 0;
}

/* When probing processes, run nvmlDeviceGetProcessUtilization to get a mapping from pid to compute
   and memory utilization (integer percent).  Also run xnvmlDeviceGetMemoryInfo to get memory
   information.  Tuck these data away in a global table and return the count of table elements.

   When extracting information, compute memory usage from total memory and memory utilization in
   the returned structure.

   The GPU has no knowledge of user IDs or command names for the pid - these have to be supplied
   by the caller.

   NOTE: The code is probably wrong or incomplete for MIG mode, more investigation needed.  In MIG
   mode, some of the APIs are simply not supported.  We can get some information with
   nvmlDeviceGetComputeRunningProcesses_v3(), but it's not clear how much information is actually
   available to unprivileged processes: the device may be locked down.  In either case the handle
   should then be a MIG handle, not a device handle.
 */

static struct nvml_gpu_process* infos;  /* NULL for no info yet */
static unsigned info_count = 0;

/* Probe the last five seconds only, both for the sake of efficiency and because sonar is supposed
   to be a sampler.  It's arguable that we could do better if we were to use a larger window, but
   sonar does not know what its own sampling window is.
*/
#define PROBE_WINDOW_SECS 5

int nvml_device_probe_processes(uint32_t device, uint32_t* count) {
    if (infos != NULL) {
        return -1;
    }
    if (load_nvml() == -1) {
        return -1;
    }

    nvmlDevice_t dev;
    if (xnvmlDeviceGetHandleByIndex_v2(device, &dev) != 0) {
        return -1;
    }

    unsigned running_procs_count = 0;
    xnvmlDeviceGetComputeRunningProcesses_v3(dev, &running_procs_count, NULL);

    nvmlProcessInfo_t *running_procs = NULL;
    if (running_procs_count > 0) {
        running_procs = malloc(sizeof(nvmlProcessInfo_t)*running_procs_count);
        if (running_procs == NULL) {
            return -1;
        }
        xnvmlDeviceGetComputeRunningProcesses_v3(dev, &running_procs_count, running_procs);
    }

    unsigned long long t = (unsigned long long)(time(NULL) - PROBE_WINDOW_SECS) * 1000000;

    unsigned utilized_procs_count = 0;
    xnvmlDeviceGetProcessUtilization(dev, NULL, &utilized_procs_count, t);

    nvmlProcessUtilizationSample_t* utilized_procs = NULL;
    if (utilized_procs_count > 0) {
        utilized_procs = malloc(sizeof(nvmlProcessUtilizationSample_t)*utilized_procs_count);
        if (utilized_procs == NULL) {
            free(running_procs);
            return -1;
        }
        xnvmlDeviceGetProcessUtilization(dev, utilized_procs, &utilized_procs_count, t);
    }

    nvmlMemory_t mem;
    xnvmlDeviceGetMemoryInfo(dev, &mem);

    info_count = 0;
    infos = malloc(sizeof(struct nvml_gpu_process)*(running_procs_count+utilized_procs_count));
    if (infos == NULL) {
        free(running_procs);
        free(utilized_procs);
        return -1;
    }
    for ( unsigned i = 0 ; i < running_procs_count ; i++ ) {
        infos[i].pid = running_procs[i].pid;
        infos[i].mem_size = running_procs[i].usedGpuMemory / 1024;
    }
    info_count = running_procs_count;
    for ( unsigned i = 0 ; i < utilized_procs_count ; i++ ) {
        unsigned j;
        for ( j = 0 ; j < info_count && infos[j].pid != utilized_procs[i].pid ; j++ ) {
        }
        if (j == info_count) {
            infos[j].pid = utilized_procs[i].pid;
            infos[j].mem_size = (utilized_procs[i].memUtil * mem.used) / 100 / 1024;
            info_count++;
        }
        infos[j].mem_util = utilized_procs[i].memUtil;
        infos[j].gpu_util = utilized_procs[i].smUtil;
    }

    free(running_procs);
    free(utilized_procs);

    *count = info_count;
    return 0;
}

int nvml_get_process(uint32_t index, struct nvml_gpu_process* infobuf) {
    if (infos == NULL) {
        return -1;
    }
    if (index >= info_count) {
        return -1;
    }

    memcpy(infobuf, infos+index, sizeof(struct nvml_gpu_process));
    return 0;
}

void nvml_free_processes() {
    if (infos != NULL) {
        free(infos);
        infos = NULL;
    }
}
