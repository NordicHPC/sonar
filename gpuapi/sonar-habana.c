/* Static-linkable wrapper around the Habana SMI dynamic library with some abstractions for our
   needs.  See sonar-habana.h and Makefile for more.

   Must be compiled with SONAR_HABANA_GPU, or there will be no support, only a stub.
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

#include "sonar-habana.h"
#include "strtcpy.h"

#ifdef SONAR_HABANA_GPU

/* https://docs.habana.ai/en/latest/Management_and_Monitoring/HLML_API/index.html */
#include <habanalabs/hlml.h>

static hlml_return_t (*s_hlml_device_get_clock_info)(hlml_device_t, hlml_clock_type_t, unsigned*);
static hlml_return_t (*s_hlml_device_get_count)(unsigned*);
static hlml_return_t (*s_hlml_device_get_handle_by_index)(unsigned, hlml_device_t*);
static hlml_return_t (*s_hlml_device_get_max_clock_info)(hlml_device_t, hlml_clock_type_t, unsigned*);
static hlml_return_t (*s_hlml_device_get_memory_info)(hlml_device_t, hlml_memory_t*);
static hlml_return_t (*s_hlml_device_get_name)(hlml_device_t, char* buf, unsigned bufsiz);
static hlml_return_t (*s_hlml_device_get_pci_info)(hlml_device_t, hlml_pci_info_t*);
static hlml_return_t (*s_hlml_device_get_performance_state)(hlml_device_t, hlml_p_states_t*);
static hlml_return_t (*s_hlml_device_get_power_management_limit)(hlml_device_t, unsigned* max);
static hlml_return_t (*s_hlml_device_get_power_usage)(hlml_device_t, unsigned*);
static hlml_return_t (*s_hlml_device_get_process_utilization)(hlml_device_t, hlml_process_utilization_sample_t*);
static hlml_return_t (*s_hlml_device_get_temperature)(hlml_device_t, hlml_temperature_sensors_t, unsigned*);
static hlml_return_t (*s_hlml_device_get_uuid)(hlml_device_t, char* buf, unsigned bufsiz);
static hlml_return_t (*s_hlml_get_driver_version)(char* buf, unsigned bufsiz);
static hlml_return_t (*s_hlml_get_fw_os_version)(hlml_device_t, char* buf, unsigned bufsiz);
static hlml_return_t (*s_hlml_init)(void);

/* Number of devices or -1 */
static int num_gpus = -1;

/* Mapping from device index to device handle */
static hlml_device_t *devs;

static void probe_gpus() {
    if (num_gpus != -1) {
        return;
    }
    unsigned n;
    if (s_hlml_device_get_count(&n) != 0) {
        return;
    }
    if (n > 0) {
        if (devs != NULL) {
            free(devs);
        }
        devs = calloc(n, sizeof(hlml_device_t));
        if (devs == NULL) {
            return;
        }
        for ( unsigned i=0 ; i < n ; i++ ) {
            if (s_hlml_device_get_handle_by_index(i, &devs[i]) != 0) {
                /*printf("Failed to get device handle\n");*/
                return;
            }
        }
    }
    num_gpus = (int)n;
}

static int load_smi() {
    static void* habana_lib;

    if (habana_lib != NULL) {
        return 0;
    }

    /* This is the location of the library on the only eX3 node that has Habana. */
    habana_lib = dlopen("/lib/habanalabs/libhlml.so", RTLD_NOW);
    if (habana_lib == NULL) {
        /*printf("Could not load library\n");*/
        return -1;
    }
    /*printf("Loaded lib, %p\n", habana_lib);*/

#define DLSYM(var, str)                         \
    if ((var = dlsym(habana_lib, str)) == NULL) {      \
        /*puts(str);*/                                 \
        habana_lib = NULL;                             \
        return -1;                              \
    }

    DLSYM(s_hlml_device_get_clock_info, "hlml_device_get_clock_info");
    DLSYM(s_hlml_device_get_count, "hlml_device_get_count");
    DLSYM(s_hlml_device_get_handle_by_index, "hlml_device_get_handle_by_index");
    DLSYM(s_hlml_device_get_max_clock_info, "hlml_device_get_max_clock_info");
    DLSYM(s_hlml_device_get_memory_info, "hlml_device_get_memory_info");
    DLSYM(s_hlml_device_get_name, "hlml_device_get_name");
    DLSYM(s_hlml_device_get_pci_info, "hlml_device_get_pci_info");
    DLSYM(s_hlml_device_get_performance_state, "hlml_device_get_performance_state");
    DLSYM(s_hlml_device_get_power_management_limit, "hlml_device_get_power_management_limit");
    DLSYM(s_hlml_device_get_power_usage, "hlml_device_get_power_usage");
    DLSYM(s_hlml_device_get_process_utilization, "hlml_device_get_process_utilization");
    DLSYM(s_hlml_device_get_temperature, "hlml_device_get_temperature");
    DLSYM(s_hlml_device_get_uuid, "hlml_device_get_uuid");
    DLSYM(s_hlml_get_driver_version, "hlml_get_driver_version");
    DLSYM(s_hlml_get_fw_os_version, "hlml_get_fw_os_version");
    DLSYM(s_hlml_init, "hlml_init");

    int init_result = s_hlml_init();
    if (init_result != 0) {
        /*printf("Could not init library\n");*/
        habana_lib = NULL;
        return -1;
    }

    probe_gpus();
    if (num_gpus == -1 || (num_gpus > 0 && devs == NULL)) {
        /*printf("Could not probe GPUs\n");*/
        habana_lib = NULL;
        return -1;
    }

    return 0;
}

#endif /* SONAR_HABANA_GPU */

int habana_device_get_count(uint32_t* count) {
#ifdef SONAR_HABANA_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }

    *count = (uint32_t)num_gpus;
    return 0;
#else
    return -1;
#endif /* SONAR_HABANA_GPU */
}

int habana_device_get_card_info(uint32_t device_index, struct habana_card_info_t* infobuf) {
#ifdef SONAR_HABANA_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    memset(infobuf, 0, sizeof(*infobuf));
    {
        hlml_pci_info_t info;
        if (s_hlml_device_get_pci_info(devs[device_index], &info) == 0) {
            strtcpy(infobuf->bus_addr, info.bus_id, sizeof(infobuf->bus_addr));
        }
    }
    s_hlml_device_get_name(devs[device_index], infobuf->model, (unsigned)sizeof(infobuf->model));
    /* There are other clock options, I'm only guessing this is the one we want */
    s_hlml_device_get_max_clock_info(devs[device_index], HLML_CLOCK_SOC, &infobuf->max_ce_clock);
    {
        hlml_memory_t memory;
        if (s_hlml_device_get_memory_info(devs[device_index], &memory) == 0) {
            infobuf->totalmem = (uint64_t)memory.total;
        }
    }
    s_hlml_device_get_uuid(devs[device_index], infobuf->uuid, (unsigned)sizeof(infobuf->uuid));
    s_hlml_get_driver_version(infobuf->driver, (unsigned)sizeof(infobuf->driver));
    s_hlml_get_fw_os_version(devs[device_index], infobuf->firmware, (unsigned)sizeof(infobuf->firmware));
    if (strcmp(infobuf->firmware, "N/A") == 0) {
        char fn[256];
        snprintf(fn, sizeof(fn), "/sys/class/accel/accel%d/device/armcp_ver", device_index);
        FILE* fp = fopen(fn, "r");
        if (fp != NULL) {
            if (fgets(infobuf->firmware, sizeof(infobuf->firmware), fp) != NULL) {
                size_t n = strlen(infobuf->firmware);
                if (n > 0 && infobuf->firmware[n-1] == '\n') {
                    infobuf->firmware[n-1] = 0;
                }
            }
            fclose(fp);
        }
    }
    s_hlml_device_get_power_management_limit(devs[device_index], &infobuf->max_power_limit);
    infobuf->max_power_limit /= 1000; /* milliWatt -> Watt */
    return 0;
#else
    return -1;
#endif /* SONAR_HABANA_GPU */
}

int habana_device_get_card_state(uint32_t device_index, struct habana_card_state_t* infobuf) {
#ifdef SONAR_HABANA_GPU
    load_smi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device_index >= (uint32_t)num_gpus) {
        return -1;
    }

    memset(infobuf, 0, sizeof(*infobuf));
    /* TODO: There are various temperature options here, "AIP" is most likely */
    s_hlml_device_get_temperature(devs[device_index], HLML_TEMPERATURE_ON_AIP, &infobuf->temp);
    {
        hlml_memory_t memory;
        if (s_hlml_device_get_memory_info(devs[device_index], &memory) == 0) {
            infobuf->mem_used = (uint64_t)memory.used;
            infobuf->mem_util = (float)((uint64_t)memory.used * 100 / (uint64_t)memory.total);
        }
    }
    {
        hlml_process_utilization_sample_t util;
        if (s_hlml_device_get_process_utilization(devs[device_index], &util) == 0) {
            infobuf->gpu_util = (float)util.aip_util;
        }
    }
    /* TODO: There are other clocks */
    s_hlml_device_get_clock_info(devs[device_index], HLML_CLOCK_SOC, &infobuf->ce_clock);
    s_hlml_device_get_power_usage(devs[device_index], &infobuf->power);
    infobuf->power /= 1000;  /* milliWatt -> Watt */
    {
        hlml_p_states_t pstate;
        if (s_hlml_device_get_performance_state(devs[device_index], &pstate) == 0) {
            if (pstate == HLML_PSTATE_UNKNOWN) {
                infobuf->perf_state = PERF_STATE_UNKNOWN;
            } else {
                infobuf->perf_state = (int)(pstate - HLML_PSTATE_0);
            }
        }
    }

    return 0;
#else
    return -1;
#endif /* SONAR_HABANA_GPU */
}
