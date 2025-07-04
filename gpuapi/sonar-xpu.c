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
static xpum_result_t (*xpu_init)(void);
static xpum_result_t (*xpu_shut_down)(void);
static xpum_result_t (*xpu_get_device_list)(xpum_device_basic_info* devices, int* count);
static xpum_result_t (*xpu_get_device_properties)(xpum_device_id_t device, xpum_device_properties_t* props);
static xpum_result_t (*xpu_get_device_power_limits)(xpum_device_id_t device, int32_t tile_id, xpum_power_limits_t* limits);
static xpum_result_t (*xpu_get_stats)(xpum_device_id_t device, xpum_device_stats_t* data_list, uint32_t* count,
                                      uint64_t* begin, uint64_t* end, uint64_t session_id);
static xpum_result_t (*xpu_get_device_utilization_by_process)(xpum_device_id_t device, uint32_t util_interval,
                                                              xpum_device_util_by_process_t* data_array, uint32_t *count);
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

    DLSYM(xpu_init, "xpumInit");
    DLSYM(xpu_shut_down, "xpumShutdown");
    DLSYM(xpu_get_device_list, "xpumGetDeviceList");
    DLSYM(xpu_get_device_properties, "xpumGetDeviceProperties");
    DLSYM(xpu_get_device_power_limits, "xpumGetDevicePowerLimits");
    DLSYM(xpu_get_stats, "xpumGetStats");
    DLSYM(xpu_get_device_utilization_by_process, "xpumGetDeviceUtilizationByProcess");

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
#if 0
          /* At least on the Simula eX3 node, the UUID is basically just the bus address, which is
             not unique enough.  We could fall bak on XPUM_DEVICE_PROPERTY_SERIAL_NUMBER and/or
             XPUM_DEVICE_PROPERTY_VENDOR_NAME but they too have non-unique / uninteresting values.
             So we synthesize a better UUID below. */
        case XPUM_DEVICE_PROPERTY_UUID:
          strtcpy(infobuf->uuid, props.properties[i].value, sizeof(infobuf->uuid)-1);
          break;
#endif
        case XPUM_DEVICE_PROPERTY_MEMORY_PHYSICAL_SIZE_BYTE:
          infobuf->totalmem = (uint64_t)strtoull(props.properties[i].value, NULL, 10);
          break;
        case XPUM_DEVICE_PROPERTY_CORE_CLOCK_RATE_MHZ:
          infobuf->max_ce_clock = (uint64_t)strtoull(props.properties[i].value, NULL, 10);
          break;
        default:
          break;
      }

      /* NOTE: The firmware info is basically not useful (not the other property groups either,
       * _AMC_ and _DATA_) on the devices I have access to on the Simula eX3 cluster.
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

    /* We must synthesize a UUID here, as the devices do not provide UUIDs that are unique in any
       interesting way.  We use bus address + node FQDN + boot time.  This will lead to having a lot
       of "different" cards over time but guarantees that there is no confusing them.  It should
       never happen that we cannot obtain host name or boot time. */

    char hostname[65];
    *hostname = 0;
    gethostname(hostname, sizeof(hostname));

    /* The boot time is first field of the `btime` line of /proc/stat, it is measured in seconds
       since epoch.  This file can have some very long lines before we get to `btime`. */
    char boot_time[65];
    *boot_time = 0;
    FILE* fp = fopen("/proc/stat", "r");
    if (fp != NULL) {
        int found = 0;
        char* p = boot_time;
        while (!found) {
            /* At the top of the loop, we're always at the start of a line. */
            int c = fgetc(fp);
            if (c == EOF) {
                break;
            }
            if (c == 'b') {
                if (fgetc(fp) == 't' && fgetc(fp) == 'i' && fgetc(fp) == 'm' && fgetc(fp) == 'e' && fgetc(fp) == ' ') {
                    found = true;
                }
            }
            while ((c = fgetc(fp)) != EOF && c != '\n') {
                if (found) {
                    *p++ = c;
                }
            }
        }
        *p = 0;
        fclose(fp);
    }

    /* Use "/" to separate the fields so that code that wants to hack around the proliferation of
       device UUIDs has something to work with.  A "/" is not legal within any of the fields. */
    snprintf(infobuf->uuid, sizeof(infobuf->uuid), "%s/%s/%s", hostname, boot_time, infobuf->bus_addr);

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

#ifdef SONAR_XPU_GPU
static struct xpu_gpu_process_t* infos;  /* NULL for no info yet */
static unsigned info_count = 0;
#endif

int xpu_device_probe_processes(uint32_t device_index, uint32_t* count) {
#ifdef SONAR_XPU_GPU
    if (infos != NULL) {
        return -1;
    }
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    xpum_device_properties_t props;
    if (xpu_get_device_properties(devs[device_index].deviceId, &props) != 0) {
        return -1;
    }
    uint64_t totalMem = 0;
    for (int i=0 ; i < props.propertyLen ; i++) {
        if (props.properties[i].name == XPUM_DEVICE_PROPERTY_MEMORY_PHYSICAL_SIZE_BYTE) {
            totalMem = (uint64_t)strtoull(props.properties[i].value, NULL, 10);
            break;
        }
    }
    if (totalMem == 0) {
        return -1;
    }

    /* The API is "if at first you don't succeed; try, try again" */
    uint32_t procCount = 0;
    xpum_device_util_by_process_t* stats;
    {
        uint32_t try = 5;
        xpum_result_t r;
        for (;;) {
            procCount = try;
            stats = calloc(procCount, sizeof(xpum_device_util_by_process_t));
            if (stats == NULL) {
                return -1;
            }
            r = xpu_get_device_utilization_by_process(devs[device_index].deviceId, 100*1000, stats, &procCount);
            if (r != XPUM_BUFFER_TOO_SMALL) {
                break;
            }
            free(stats);
            stats = NULL;
            try *= 2;
        }
        if (r != XPUM_OK) {
            free(stats);
            return -1;
        }
    }

    infos = calloc(procCount, sizeof(struct xpu_gpu_process_t));
    if (infos == NULL) {
        free(stats);
        return -1;
    }
    info_count = procCount;

    for (uint32_t p = 0 ; p < procCount ; p++ ) {
        infos[p].pid = stats[p].processId;
        infos[p].gpu_util = stats[p].computeEngineUtil;
        infos[p].mem_util = stats[p].memSize * 100 / totalMem;
        infos[p].mem_size = stats[p].memSize / 1024;
    }

    *count = info_count;
    free(stats);
    return 0;
#else
    return -1;
#endif
}

int xpu_get_process(uint32_t process_index, struct xpu_gpu_process_t* infobuf) {
#ifdef SONAR_XPU_GPU
    if (infos == NULL) {
        return -1;
    }
    if (process_index >= info_count) {
        return -1;
    }
    memcpy(infobuf, infos+process_index, sizeof(struct xpu_gpu_process_t));
    return 0;
#else
    return -1;
#endif
}

void xpu_free_processes() {
#ifdef SONAR_XPU_GPU
    if (infos != NULL) {
        free(infos);
        infos = NULL;
    }
#endif
}
