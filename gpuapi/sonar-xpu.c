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
#include "strtcpy.h"

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
static xpum_result_t (*xpu_get_device_properties)(xpum_device_id_t device, xpum_device_properties_t* props);
//static xpum_result_t (*xpu_get_health_config)(xpum_device_id_t device, xpum_health_config_type_t key, void* value);
static xpum_result_t (*xpu_get_device_power_limits)(xpum_device_id_t device, int32_t tileId, xpum_power_limits_t* limits);
static xpum_result_t (*xpu_get_stats)(xpum_device_id_t device, xpum_device_stats_t* dataList, uint32_t* count,
                                      uint64_t* begin, uint64_t* end, uint64_t session_id);

static int num_gpus = -1;

#ifdef SONAR_XPU_GPU
/* Canonical mapping from device index to device information */
static xpum_device_basic_info *devs;
#endif

static void probe_gpus() {
    if (num_gpus != -1) {
        return;
    }
    if (xpu_get_device_list(NULL, &num_gpus) != 0) {
        num_gpus = 0;
    }

    if (num_gpus > 0) {
        devs = calloc(num_gpus, sizeof(xpum_device_basic_info));
        if (devs == NULL) {
            return;
        }
        int count = num_gpus;
        if (xpu_get_device_list(devs, &count) != 0) {
            free(devs);
            devs = NULL;
            return;
        }
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
        /*printf("Could not load library\n");*/
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
    DLSYM(xpu_get_device_properties, "xpumGetDeviceProperties");
//    DLSYM(xpu_get_health_config, "xpumGetHealthConfig");
    DLSYM(xpu_get_device_power_limits, "xpumGetDevicePowerLimits");
    DLSYM(xpu_get_stats, "xpumGetStats");

    /* You'd think that passing parameters would be better, but no. */
    setenv("XPUM_DISABLE_PERIODIC_METRIC_MONITOR", "1", 1);
    setenv("XPUM_METRICS", "0,4,6,7,8,9", 1);

    /* Silence logging during init */
    int tmp = dup(1);
    int null = open("/dev/null", O_WRONLY);
    dup2(null, 1);
    int init_result = xpu_init();
    dup2(tmp, 1);
    close(tmp);
    /* Init done */

    if (init_result != 0) {
        printf("Could not init library\n");
        lib = NULL;
        return -1;
    }

    probe_gpus();
    if (num_gpus == -1 || (num_gpus > 0 && devs == NULL)) {
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

int xpu_device_get_card_info(uint32_t device_index, struct xpu_card_info_t* infobuf) {
#ifdef SONAR_XPU_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    memset(infobuf, 0, sizeof(*infobuf));
    xpum_device_properties_t props;
    xpu_get_device_properties(devs[device_index].deviceId, &props);
    int firmware_name_index = -1;
    int firmware_version_index = -1;
    for (int i=0 ; i < props.propertyLen ; i++ ) {
      switch (props.properties[i].name) {
        /* The order here is as in the struct */
        case XPUM_DEVICE_PROPERTY_PCI_BDF_ADDRESS:
          strtcpy(infobuf->bus_addr, props.properties[i].value, sizeof(infobuf->bus_addr)-1);
          break;
        case XPUM_DEVICE_PROPERTY_DEVICE_NAME:
          strtcpy(infobuf->model, props.properties[i].value, sizeof(infobuf->model)-1);
          break;
        case XPUM_DEVICE_PROPERTY_DRIVER_VERSION:
          strtcpy(infobuf->driver, props.properties[i].value, sizeof(infobuf->driver)-1);
          break;
        case XPUM_DEVICE_PROPERTY_GFX_DATA_FIRMWARE_NAME:
          firmware_name_index = i;
          break;
        case XPUM_DEVICE_PROPERTY_GFX_DATA_FIRMWARE_VERSION:
          firmware_version_index = i;
          break;
        case XPUM_DEVICE_PROPERTY_UUID:
          /* FIXME: At least on the eX3 node, the UUID is basically just the bus address, which is
             not unique enough.  We could fall bak on XPUM_DEVICE_PROPERTY_SERIAL_NUMBER and/or
             XPUM_DEVICE_PROPERTY_VENDOR_NAME but they too have non-unique / uninteresting values.
             So we might use bus address + boot time, which will lead to a lot of different "cards"
             over time but guarantees that there is no overlap in the identity, unlike now.  */
          strtcpy(infobuf->uuid, props.properties[i].value, sizeof(infobuf->uuid)-1);
          break;
        case XPUM_DEVICE_PROPERTY_MEMORY_PHYSICAL_SIZE_BYTE:
          infobuf->totalmem = (uint64_t)strtoull(props.properties[i].value, NULL, 10);
          break;
        case XPUM_DEVICE_PROPERTY_CORE_CLOCK_RATE_MHZ:
          infobuf->max_ce_clock = (uint64_t)strtoull(props.properties[i].value, NULL, 10);
          break;
        default:
          break;
      }

      /* The firmware info is basically not useful (not any of the other properties either, _AMC_
       * and not _DATA_) on the devices I have access to.
       */
      if (firmware_name_index >= 0 && firmware_version_index >= 0) {
          snprintf(infobuf->firmware,
                   sizeof(infobuf->firmware),
                   "%s @ %s",
                   props.properties[firmware_name_index].value,
                   props.properties[firmware_version_index].value);
      } else if (firmware_name_index >= 0) {
          strtcpy(infobuf->firmware, props.properties[firmware_name_index].value, sizeof(infobuf->firmware)-1);
      } else if (firmware_version_index >= 0) {
          strtcpy(infobuf->firmware, props.properties[firmware_version_index].value, sizeof(infobuf->firmware)-1);
      }

      {
          xpum_power_limits_t limits;
          xpu_get_device_power_limits(devs[device_index].deviceId, -1, &limits);
          infobuf->max_power_limit = (unsigned)limits.sustained_limit.power / 1000;
      }
    }

    return 0;
#else
    return -1;
#endif /* SONAR_XPU_GPU */
}

int xpu_device_get_card_state(uint32_t device_index, struct xpu_card_state_t* infobuf) {
#ifdef SONAR_XPU_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    memset(infobuf, 0, sizeof(*infobuf));
    uint32_t count;
    uint64_t begin, end;
    if (xpu_get_stats(devs[device_index].deviceId, NULL, &count, &begin, &end, 0) != 0) {
        return -1;
    }
    xpum_device_stats_t *stats = calloc(count, sizeof(xpum_device_stats_t));
    if (stats == NULL) {
        return -1;
    }
    if (xpu_get_stats(devs[device_index].deviceId, stats, &count, &begin, &end, 0) != 0) {
        free(stats);
        return -1;
    }

    /* This is a fairly bizarre interface.  It's totally not obvious why there should ever be more
       than one element in the outer array.  It may be a concession to xpumGetStatsEx() which can
       take more than one device ID.

       To make sense of this, we'll iterate over the outer array and take the first element that
       matches our device ID.  This is probably overly cautious.
    */
    for (uint32_t c=0 ; c < count ; c++ ) {
        if (stats[c].deviceId == devs[device_index].deviceId) {
            xpum_device_stats_t *s = &stats[c];
            for (int32_t i=0 ; i < s->count ; i++ ) {
                xpum_device_stats_data_t *d = &s->dataList[i];
                switch (d->metricsType) {
                  case XPUM_STATS_GPU_UTILIZATION:
                    infobuf->gpu_util = (float)((double)d->value / (double)d->scale);
                    break;
                  case XPUM_STATS_POWER:
                    infobuf->power = (unsigned)(d->value / d->scale);
                    break;
                  case XPUM_STATS_GPU_FREQUENCY:
                    infobuf->ce_clock = (unsigned)d->value;
                    break;
                  case XPUM_STATS_GPU_CORE_TEMPERATURE:
                    /* Not available on the ex3 node I have */
                    infobuf->temp = (unsigned)d->value;
                    break;
                  case XPUM_STATS_MEMORY_USED:
                    infobuf->mem_used = d->value;
                    break;
                  case XPUM_STATS_MEMORY_UTILIZATION:
                    infobuf->mem_util = (float)((double)d->value / (double)d->scale);
                    break;
                  default:
                    break;
                }
            }
            break;
        }
    }

    free(stats);
    return 0;
#else
    return -1;
#endif /* SONAR_XPU_GPU */
}

