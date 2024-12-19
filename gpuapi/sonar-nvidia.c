/* Remember to `module load CUDA/11.1.1-GCC-10.2.0` or similar for nvml.h.

   On the UiO ML nodes, header files are here:
     /storage/software/CUDA/11.3.1/targets/x86_64-linux/include/nvml.h
     /storage/software/CUDAcore/11.1.1/targets/x86_64-linux/include/nvml.h
*/

#include <stddef.h>
#include <string.h>
#include <inttypes.h>
#include <stdio.h> /* snprintf, we can fix this */
#include <dlfcn.h>
#include <nvml.h>

#include "sonar-nvidia.h"

static nvmlReturn_t (*xnvmlDeviceGetClockInfo)(nvmlDevice_t,nvmlClockType_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetComputeMode)(nvmlDevice_t,nvmlComputeMode_t*);
static nvmlReturn_t (*xnvmlDeviceGetCount_v2)(unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetHandleByIndex_v2)(int index, nvmlDevice_t* dev);
static nvmlReturn_t (*xnvmlDeviceGetArchitecture)(nvmlDevice_t, nvmlDeviceArchitecture_t*);
static nvmlReturn_t (*xnvmlDeviceGetFanSpeed)(nvmlDevice_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetMemoryInfo)(nvmlDevice_t, nvmlMemory_t*);
static nvmlReturn_t (*xnvmlDeviceGetMaxClockInfo)(nvmlDevice_t,nvmlClockType_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetName)(nvmlDevice_t,char*,unsigned);
static nvmlReturn_t (*xnvmlDeviceGetPciInfo_v3)(nvmlDevice_t,nvmlPciInfo_t*);
static nvmlReturn_t (*xnvmlDeviceGetPerformanceState)(nvmlDevice_t,nvmlPstates_t*);
static nvmlReturn_t (*xnvmlDeviceGetPowerManagementLimitConstraints)(nvmlDevice_t,unsigned*,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetPowerManagementLimit)(nvmlDevice_t,unsigned*);
static nvmlReturn_t (*xnvmlDeviceGetPowerUsage)(nvmlDevice_t,unsigned*);
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
    DLSYM(xnvmlDeviceGetCount_v2, "nvmlDeviceGetCount_v2");
    DLSYM(xnvmlDeviceGetHandleByIndex_v2, "nvmlDeviceGetHandleByIndex_v2");
    DLSYM(xnvmlDeviceGetArchitecture, "nvmlDeviceGetArchitecture");
    DLSYM(xnvmlDeviceGetFanSpeed, "nvmlDeviceGetFanSpeed");
    DLSYM(xnvmlDeviceGetMemoryInfo, "nvmlDeviceGetMemoryInfo");
    DLSYM(xnvmlDeviceGetMaxClockInfo, "nvmlDeviceGetMaxClockInfo");
    DLSYM(xnvmlDeviceGetName, "nvmlDeviceGetName");
    DLSYM(xnvmlDeviceGetPciInfo_v3, "nvmlDeviceGetPciInfo_v3");
    DLSYM(xnvmlDeviceGetPerformanceState, "nvmlDeviceGetPerformanceState");
    DLSYM(xnvmlDeviceGetPowerManagementLimitConstraints, "nvmlDeviceGetPowerManagementLimitConstraints");
    DLSYM(xnvmlDeviceGetPowerManagementLimit, "nvmlDeviceGetPowerManagementLimit");
    DLSYM(xnvmlDeviceGetPowerUsage, "nvmlDeviceGetPowerUsage");
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
    xnvmlDeviceGetPowerManagementLimitConstraints(dev, &infobuf->min_power_limit, &infobuf->max_power_limit);

    int cuda;
    if (xnvmlSystemGetCudaDriverVersion(&cuda) == 0) {
        snprintf(infobuf->firmware, sizeof(infobuf->firmware), "%d.%d",
                 NVML_CUDA_DRIVER_VERSION_MAJOR(cuda),
                 NVML_CUDA_DRIVER_VERSION_MINOR(cuda));
    }

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
    // etc

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
        // Note, only the "Default" string is known to match nvidia-smi.
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

// The requirement here is that we should also see orphaned processes.
//
// In terms of the nvml API:
//
//  - nvmlDeviceGetProcessUtilization() is like pmon and can get per-pid utilization
//  - nvmlDeviceGetComputeRunningProcesses_v3() will return a vector
//    of running processes, with pid and used memories.
//
// It's unclear if these two together are sufficient to get information about orphaned
// processes but it's a start.
//
// Possibly nvmlDeviceGetProcessesUtilizationInfo() is really the better API?
//
// MIG: Of the three, only nvmlDeviceGetComputeRunningProcesses_v3() is supported on MIG-enabled
// GPUs, and here information about other users' processes may not be available to unprivileged
// users.
//
// In either case, this will probably have some kind of setup / lookup / cleanup API,
// so that any memory management can be confined to the GPU layer.
//
// Not yet clear how to discover whether a node / card is in MIG mode.

int nvml_device_probe_processes(uint32_t device, uint32_t* count) {
    // FIXME
    return -1;
}

int nvml_get_process(uint32_t index, struct nvml_gpu_process* infobuf) {
    // FIXME
    return -1;
}

int nvml_free_processes() {
    // FIXME
    return -1;
}
