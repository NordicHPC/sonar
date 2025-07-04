/* Static-linkable wrapper around the XPU SMI dynamic library with some abstractions for our
   needs.  See sonar-xpu.h and Makefile for more.

   Must be compiled with SONAR_XPU_GPU, or there will be no support, only a stub.
*/

#include <assert.h>
#include <dlfcn.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>

#include "sonar-xpu.h"

#ifdef SONAR_XPU_GPU

#include "xpum_api.h"
#include "xpum_structs.h"

/* Note that these variably take size_t and uint32_t for the buffer length parameter, do not copy
   prototypes indiscriminately.
*/
#if 0
static rsmi_status_t (*xrsmi_compute_process_info_by_pid_get)(uint32_t, rsmi_process_info_t*);
static rsmi_status_t (*xrsmi_compute_process_info_get)(rsmi_process_info_t*, uint32_t*);
static rsmi_status_t (*xrsmi_compute_process_gpus_get)(uint32_t, uint32_t*, uint32_t*);
static rsmi_status_t (*xrsmi_dev_busy_percent_get)(uint32_t, uint32_t*);
static rsmi_status_t (*xrsmi_dev_current_socket_power_get)(uint32_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_fan_speed_get)(uint32_t, uint32_t, int64_t*);
static rsmi_status_t (*xrsmi_dev_firmware_version_get)(uint32_t, rsmi_fw_block_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_gpu_clk_freq_get)(uint32_t, rsmi_clk_type_t, rsmi_frequencies_t*);
static rsmi_status_t (*xrsmi_dev_guid_get)(uint32_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_memory_busy_percent_get)(uint32_t, uint32_t*);
static rsmi_status_t (*xrsmi_dev_memory_total_get)(uint32_t, rsmi_memory_type_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_memory_usage_get)(uint32_t, rsmi_memory_type_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_name_get)(uint32_t, char*, size_t);
static rsmi_status_t (*xrsmi_dev_pci_id_get)(uint32_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_perf_level_get)(uint32_t, rsmi_dev_perf_level_t*);
static rsmi_status_t (*xrsmi_dev_power_cap_get)(uint32_t, uint32_t, uint64_t*);
static rsmi_status_t (*xrsmi_dev_power_cap_range_get)(uint32_t, uint32_t, uint64_t*, uint64_t*);
static rsmi_status_t (*xrsmi_dev_serial_number_get)(uint32_t, char*, uint32_t);
static rsmi_status_t (*xrsmi_dev_temp_metric_get)(uint32_t, uint32_t, rsmi_temperature_metric_t, int64_t*);
static rsmi_status_t (*xrsmi_dev_unique_id_get)(uint32_t, uint64_t*);
static rsmi_status_t (*xrsmi_init)(uint64_t flags);
static rsmi_status_t (*xrsmi_num_monitor_devices)(uint32_t*);
static rsmi_status_t (*xrsmi_shut_down)(void);
static rsmi_status_t (*xrsmi_version_str_get)(rsmi_sw_component_t, char*, uint32_t);
static rsmi_status_t (*xrsmi_utilization_count_get)(uint32_t,
                                                    rsmi_utilization_counter_t*,
                                                    uint32_t,
                                                    uint64_t*);
#endif
static xpum_result_t (*xpu_init)(void);
static xpum_result_t (*xpu_shut_down)(void);
static xpum_result_t (*xpu_get_device_list)(xpum_device_basic_info* devices, int* count);

static int num_gpus = -1;

static void probe_gpus() {
    if (num_gpus != -1) {
        return;
    }
    if (xpu_get_device_list(NULL, &num_gpus) != 0) {
        num_gpus = 0;
    }
}

static int load_smi() {
    static void* lib;

    if (lib != NULL) {
        return 0;
    }

    /* This is the location of the library on the only eX3 node that has XPU. */
    lib = dlopen("/lib/x86_64-linux-gnu/libxpum.so.1", RTLD_NOW);
    if (lib == NULL) {
        printf("Could not load library\n");
        return -1;
    }

#define DLSYM(var, str)                         \
    if ((var = dlsym(lib, str)) == NULL) {      \
        /*puts(str);*/                          \
        lib = NULL;                             \
        return -1;                              \
    }

#if 0
    DLSYM(xrsmi_compute_process_info_by_pid_get, "rsmi_compute_process_info_by_pid_get");
    DLSYM(xrsmi_compute_process_info_get, "rsmi_compute_process_info_get");
    DLSYM(xrsmi_compute_process_gpus_get, "rsmi_compute_process_gpus_get");
    DLSYM(xrsmi_dev_busy_percent_get, "rsmi_dev_busy_percent_get");
    DLSYM(xrsmi_dev_current_socket_power_get, "rsmi_dev_current_socket_power_get");
    DLSYM(xrsmi_dev_fan_speed_get, "rsmi_dev_fan_speed_get");
    DLSYM(xrsmi_dev_firmware_version_get, "rsmi_dev_firmware_version_get");
    DLSYM(xrsmi_dev_gpu_clk_freq_get, "rsmi_dev_gpu_clk_freq_get");
    DLSYM(xrsmi_dev_guid_get, "rsmi_dev_guid_get");
    DLSYM(xrsmi_dev_memory_busy_percent_get, "rsmi_dev_memory_busy_percent_get");
    DLSYM(xrsmi_dev_memory_total_get, "rsmi_dev_memory_total_get");
    DLSYM(xrsmi_dev_memory_usage_get, "rsmi_dev_memory_usage_get");
    DLSYM(xrsmi_dev_name_get, "rsmi_dev_name_get");
    DLSYM(xrsmi_dev_pci_id_get, "rsmi_dev_pci_id_get");
    DLSYM(xrsmi_dev_perf_level_get, "rsmi_dev_perf_level_get");
    DLSYM(xrsmi_dev_power_cap_get, "rsmi_dev_power_cap_get");
    DLSYM(xrsmi_dev_power_cap_range_get, "rsmi_dev_power_cap_range_get");
    DLSYM(xrsmi_dev_serial_number_get, "rsmi_dev_serial_number_get");
    DLSYM(xrsmi_dev_temp_metric_get, "rsmi_dev_temp_metric_get");
    DLSYM(xrsmi_dev_unique_id_get, "rsmi_dev_unique_id_get");
    DLSYM(xrsmi_init, "rsmi_init");
    DLSYM(xrsmi_num_monitor_devices, "rsmi_num_monitor_devices");
    DLSYM(xrsmi_shut_down, "rsmi_shut_down");
    DLSYM(xrsmi_utilization_count_get, "rsmi_utilization_count_get");
    DLSYM(xrsmi_version_str_get, "rsmi_version_str_get");
#endif
    DLSYM(xpu_init, "xpumInit");
    DLSYM(xpu_shut_down, "xpumShutdown");
    DLSYM(xpu_get_device_list, "xpumGetDeviceList");

    /* You'd think that passing parameters would be better, but no. */
    setenv("XPUM_DISABLE_PERIODIC_METRIC_MONITOR", "1", 1);
    setenv("XPUM_METRICS", "0,4,6,7,8,9", 1);
    //printf("STARTING INIT\n");
    /* Silence logging during init */
    int tmp = dup(1);
    int null = open("/dev/null", O_WRONLY);
    dup2(null, 1);
    if (xpu_init() != 0) {
        printf("Could not init library\n");
        lib = NULL;
        return -1;
    }
    dup2(tmp, 1);
    close(tmp);
    //printf("ENDING INIT\n");

    probe_gpus();
    if (num_gpus == -1) {
        printf("Could not probe GPUs\n");
        lib = NULL;
        return -1;
    }

    return 0;
}

#endif /* SONAR_XPU_GPU */

int xpu_device_get_count(uint32_t* count) {
#ifdef SONAR_XPU_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }

    *count = (uint32_t)num_gpus;
    return 0;
#else
    return -1;
#endif /* SONAR_XPU_GPU */
}

#ifdef SONAR_XPU_GPU
static xpum_device_basic_info *devs;
#endif

int xpu_device_get_card_info(uint32_t device, struct xpu_card_info_t* infobuf) {
#ifdef SONAR_XPU_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device >= (uint32_t)num_gpus) {
        return -1;
    }

    if (devs == NULL) {
        devs = calloc(num_gpus, sizeof(xpum_device_basic_info));
        if (devs == NULL) {
            return -1;
        }
        //printf("STARTING PROBE\n");
        int count = num_gpus;
        if (xpu_get_device_list(devs, &count) != 0) {
            free(devs);
            devs = NULL;
            return -1;
        }
        //printf("ENDING PROBE");
    }

    // TODO: At least on the eX3 node, the uuid is just the bus address, which is not unique enough.

    memset(infobuf, 0, sizeof(*infobuf));
    strncpy(infobuf->bus_addr, devs[device].PCIBDFAddress, sizeof(infobuf->bus_addr));
    strncpy(infobuf->uuid, devs[device].uuid, sizeof(infobuf->uuid));
    strncpy(infobuf->model, devs[device].deviceName, sizeof(infobuf->model));

    return 0;
#else
    return -1;
#endif /* SONAR_XPU_GPU */
}
