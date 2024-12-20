/* Remember to `module load CUDA/11.1.1-GCC-10.2.0` or similar for nvml.h.

   On the UiO ML nodes, header files are here:
     /storage/software/CUDA/11.3.1/targets/x86_64-linux/include/nvml.h
     /storage/software/CUDAcore/11.1.1/targets/x86_64-linux/include/nvml.h
*/

#include <dlfcn.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h> /* snprintf, we can fix this */
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
        /* Note, only the "Default" string is known to match nvidia-smi. */
        switch (mode) {
          case NVML_COMPUTEMODE_DEFAULT:
            strcpy(infobuf->compute_mode, "Default");
            break;
          case NVML_COMPUTEMODE_PROHIBITED:
            strcpy(infobuf->compute_mode, "Prohibited");
            break;
          case NVML_COMPUTEMODE_EXCLUSIVE_PROCESS:
            strcpy(infobuf->compute_mode, "ExclusiveProcess");
            break;
          default:
            strcpy(infobuf->compute_mode, "Unknown");
            break;
        }
    }

    nvmlPstates_t pstate;
    if (xnvmlDeviceGetPerformanceState(dev, &pstate) == 0) {
        if (pstate != NVML_PSTATE_UNKNOWN) {
            snprintf(infobuf->perf_state, sizeof(infobuf->perf_state), "P%d", (int)pstate);
        } else {
            strcpy(infobuf->perf_state, "Unknown");
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

/* (The following is probably wrong for MIG mode.)

   When probing processes, run nvmlDeviceGetProcessUtilization to get a mapping from pid to compute
   and memory utilization (integer percent).  Also run xnvmlDeviceGetMemoryInfo to get memory
   information.  Tuck these data away in a global table and return the count of table elements.

   When extracting information, compute memory usage from total memory and memory utilization in
   the returned structure.

   The GPU has no knowledge of user IDs or command names for the pid - these have to be supplied
   by the caller.
 */

static nvmlProcessUtilizationSample_t* infos; /* NULL for no info yet */
static unsigned info_count;
static unsigned long long total_memory;

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

    unsigned long long t = (unsigned long long)(time(NULL) - PROBE_WINDOW_SECS) * 1000000;

    info_count = 0;
    nvmlReturn_t r = xnvmlDeviceGetProcessUtilization(dev, NULL, &info_count, t);
    if (r == NVML_SUCCESS) {
        *count = 0;
        infos = malloc(sizeof(*infos)); /* Never alloc zero elements */
        if (infos == NULL) {
            return -1;
        }
        return 0;
    }

    infos = malloc(sizeof(*infos)*info_count);
    xnvmlDeviceGetProcessUtilization(dev, infos, &info_count, t);
    *count = info_count;

    nvmlMemory_t mem;
    if (xnvmlDeviceGetMemoryInfo(dev, &mem) == 0) {
        total_memory = mem.total;
    }

    return 0;
}

int nvml_get_process(uint32_t index, struct nvml_gpu_process* infobuf) {
    if (infos == NULL) {
        printf("No array\n");
        return -1;
    }
    if (index >= info_count) {
        printf("Index OOB, %u %u\n", index, info_count);
        return -1;
    }

    infobuf->pid = infos[index].pid;
    infobuf->mem_size = (infos[index].memUtil * total_memory) / (100 * 1024);
    infobuf->mem_util = infos[index].memUtil;
    infobuf->gpu_util = infos[index].smUtil;

    return 0;
}

void nvml_free_processes() {
    if (infos != NULL) {
        free(infos);
        infos = NULL;
    }
}

#if 0

/* Sketches and notes - keeping it for future use

   The code above is probably wrong / incomplete for MIG mode, for which some of the APIs are simply
   not supported.  For MIG mode, we can get some information with
   nvmlDeviceGetComputeRunningProcesses_v3(), but it's not clear how much information is actually
   available to unprivileged processes: the device may be locked down.  In either case the handle
   should then be a MIG handle, not a device handle.

   In MIG mode, nvmlDeviceGetComputeRunningProcesses_v3() returns info other than (unsigned)-1 for
   at least one of the two MIG fields.  Also see comments inline below.
*/

static void experiment() {
    if (load_nvml() == -1) {
        return;
    }

    unsigned ndev;
    if (xnvmlDeviceGetCount_v2(&ndev) != 0) {
        printf("no devices\n");
        return;
    }

    for ( unsigned devno=0 ; devno < ndev ; devno++ ) {
        nvmlDevice_t dev;
        if (xnvmlDeviceGetHandleByIndex_v2(devno, &dev) != 0) {
            printf("%u: no handle\n", devno);
            continue;
        }

        {
            unsigned info_count = 0;
            nvmlReturn_t r = xnvmlDeviceGetComputeRunningProcesses_v3(dev, &info_count, NULL);
            if (r == NVML_SUCCESS) {
                printf("%u: no processes\n", devno);
                continue;
            }

            printf("%u: %u processes\n", devno, info_count);

            nvmlProcessInfo_t *infos = malloc(sizeof(nvmlProcessInfo_t)*info_count);
            if (xnvmlDeviceGetComputeRunningProcesses_v3(dev, &info_count, infos) != 0) {
                printf("%u: process lookup failed\n", devno);
            }

            printf("%u got %u processes\n", devno, info_count);

            for ( unsigned p = 0 ; p < info_count ; p++ ) {
                printf("  %u %llu / %u %u\n", infos[p].pid, infos[p].usedGpuMemory,
                       infos[p].computeInstanceId, infos[p].gpuInstanceId);
            }

            /* if computeInstanceId != (unsigned)-1 or gpuInstanceId != (unsigned)-1 then MIG mode
               and there may be multiple processes running on it in different partitions.  In this
               case we should really not have been using the device handle to lookup info in the
               first place as the information may be privileged, but should have been using a MIG
               handle to ask for individual partitions.  (May still be privileged?)  It's generally
               complex.

               Plus I don't like that.  The Default compute mode allows for multiple contexts per
               device, per the docs.

               otherwise the device is exclusive I guess?  */

            free(infos);
        }

#if 0
        /* This API is documented but not available as of the CUDA 12.3 header file */
        {
            nvmlProcessesUtilizationInfo_t overall;
            overall.processSamplesCount = 0;
            overall.procUtilArray = NULL;
            overall.lastSeenTimestamp = time() - 300; /* five minute window */
            nvmlReturn_t r = xnvmlDeviceGetProcessesUtilizationInfo(dev, &overall);
            if (r == NVML_SUCCESS) {
                printf("%u: no processes\n", devno);
                /* No processes */
                continue;
            }

            overall.procUtilArray = malloc(
                sizeof(nvmlProcessUtilizationInfo_v1_t)*overall.processSamplesCount);
            nvmlReturn_t r = xnvmlDeviceGetProcessesUtilizationInfo(dev, &overall);
            if (r != NVML_SUCCESS) {
                printf("%u: failed to lookup\n", devno);
                /* No processes */
                continue;
            }

            for ( unsigned pn = 0 ; pn < overall.processSamplesCount ; pn++ ) {
                printf("  %u: %u %u %u\n", devno,
                       overall.procUtilArray[pn].pid,
                       overall.procUtilArray[pn].smUtil,
                       overall.procUtilArray[pn].memUtil);
            }

            free(overall.procUtilArray);
        }
#endif

        {
            long window_sec = 5;
            nvmlProcessUtilizationSample_t *infos;
            unsigned info_count = 0;
            unsigned long long t = (unsigned long long)(time(NULL) - window_sec) * 1000000;
            nvmlReturn_t r = xnvmlDeviceGetProcessUtilization(dev, NULL, &info_count, t);
            if (r == NVML_SUCCESS) {
                printf("%u: no processes\n", devno);
                continue;
            }

            infos = malloc(sizeof(*infos)*info_count);
            xnvmlDeviceGetProcessUtilization(dev, infos, &info_count, t);

            for ( unsigned i = 0 ; i < info_count ; i++ ) {
                printf("  %u: %d %d\n", infos[i].pid, infos[i].smUtil, infos[i].memUtil);
            }

            free(infos);
        }
    }
}

#endif /* #if 0 */
