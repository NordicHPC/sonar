/* Static-linkable wrapper around the AMD SMI dynamic library with some abstractions for our
   needs.  See sonar-amd.h and Makefile for more.

   Must be compiled with SONAR_AMD_GPU, or there will be no support, only a stub.

   There are two APIs: amdsmi.h and rocm_smi.h.  These are similar but not the same, and some
   functionality in the latter appears not to be exposed in the former, AND VICE VERSA.  It's
   anyone's guess what is going to be supported long-term, I've seen noise about rocm-smi being
   abandoned in favor of amd-smi.  The copyright dates on amdsmi.h are newer than on rocm_smi.h.
   For now rocm_smi suits our needs better.

   Note you may need to `module load hipSYCL/0.9.2-GCC-11.2.0-CUDA-11.4.1` or similar for access to
   rocm_smi.h, it may not be loaded by default.

   On the UiO ML nodes, headers are here:
     /opt/rocm/include/amd_smi/amdsmi.h
     /opt/rocm/include/rocm_smi/rocm_smi.h
     (where /opt/rocm is usually a link to /opt/rocm-n.m.o)

   (Note the amdsmi.h documentation is unreliable wrt units of values reported.)
*/

#include <assert.h>
#include <dlfcn.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "sonar-amd.h"

#ifdef SONAR_AMD_GPU

#include "rocm_smi/rocm_smi.h"

/* Note that these variably take size_t and uint32_t for the buffer length parameter, do not copy
   prototypes indiscriminately.
*/
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

static int num_gpus = -1;

static void probe_gpus() {
    if (num_gpus != -1) {
        return;
    }
    uint32_t count;
    if (xrsmi_num_monitor_devices(&count) == 0) {
        num_gpus = (int)count;
    } else {
        num_gpus = 0;
    }
}

static int load_rsmi() {
    static void* lib;

    if (lib != NULL) {
        return 0;
    }

    /* This is the location of the library on all the ml4. It is also where AMD says it should be. */
    lib = dlopen("/opt/rocm/lib/librocm_smi64.so.7", RTLD_NOW);
    if (lib == NULL) {
        /*printf("Could not load library\n");*/
        return -1;
    }

#define DLSYM(var, str) \
    if ((var = dlsym(lib, str)) == NULL) {      \
        /*puts(str);*/                          \
        lib = NULL;                             \
        return -1;                              \
    }

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

    /* According to doc, 0 should mean "only AMD GPUs" and this is also what rocm_smi uses */
    if (xrsmi_init(0) != 0) {
        /*printf("Could not init library\n");*/
        lib = NULL;
        return -1;
    }

    probe_gpus();
    if (num_gpus == -1) {
        /*printf("Could not probe GPUs\n");*/
        lib = NULL;
        return -1;
    }

    return 0;
}

#endif /* SONAR_AMD_GPU */

int amdml_device_get_count(uint32_t* count) {
#ifdef SONAR_AMD_GPU
    load_rsmi();
    if (num_gpus == -1) {
        return -1;
    }

    *count = (uint32_t)num_gpus;
    return 0;
#else
    return -1;
#endif /* SONAR_AMD_GPU */
}

int amdml_device_get_card_info(uint32_t device, struct amdml_card_info_t* infobuf) {
#ifdef SONAR_AMD_GPU
    load_rsmi();
    if (num_gpus == -1) {
        return -1;
    }
    if (device >= (uint32_t)num_gpus) {
        return -1;
    }
    memset(infobuf, 0, sizeof(*infobuf));

    xrsmi_dev_name_get(device, infobuf->model, sizeof(infobuf->model)-1);
    uint64_t uuid;
    if (xrsmi_dev_unique_id_get(device, &uuid) == 0) {
        snprintf(infobuf->uuid, sizeof(infobuf->uuid), "%llx", (unsigned long long)uuid);
    }
    xrsmi_version_str_get(RSMI_SW_COMP_DRIVER, infobuf->driver, sizeof(infobuf->driver)-1);

    uint64_t cap;
    if (xrsmi_dev_power_cap_get(device, 0, &cap) == 0) {
        infobuf->power_limit = (unsigned)(cap / 1000);
    }
    uint64_t mincap, maxcap;
    if (xrsmi_dev_power_cap_range_get(device, 0, &maxcap, &mincap) == 0) {
        infobuf->min_power_limit = (unsigned)(mincap / 1000);
        infobuf->max_power_limit = (unsigned)(maxcap / 1000);
    }

    rsmi_frequencies_t freqs;
    if (xrsmi_dev_gpu_clk_freq_get(device, RSMI_CLK_TYPE_SYS, &freqs) == 0) {
        infobuf->min_ce_clock = freqs.frequency[0] / 1000000;
        infobuf->max_ce_clock = freqs.frequency[freqs.num_supported-1] / 1000000;
    }
    if (xrsmi_dev_gpu_clk_freq_get(device, RSMI_CLK_TYPE_MEM, &freqs) == 0) {
        infobuf->min_mem_clock = freqs.frequency[0] / 1000000;
        infobuf->max_mem_clock = freqs.frequency[freqs.num_supported-1] / 1000000;
    }

    xrsmi_dev_memory_total_get(device, RSMI_MEM_TYPE_VRAM, &infobuf->totalmem);
    uint64_t fw;
    if (xrsmi_dev_firmware_version_get(device, RSMI_FW_BLOCK_CE, &fw) == 0) {
        snprintf(infobuf->firmware, sizeof(infobuf->firmware), "ce=%llu", (unsigned long long)fw);
    }

    /* https://wiki.xenproject.org/wiki/Bus:Device.Function_(BDF)_Notation */
    uint64_t bdfid;
    if (xrsmi_dev_pci_id_get(device, &bdfid) == 0) {
        snprintf(infobuf->bus_addr, sizeof(infobuf->bus_addr), "%08x:%02x:%02x.%x",
                 (unsigned)(bdfid >> 32),
                 (unsigned)((bdfid >> 8) & 255),
                 (unsigned)((bdfid >> 3) & 15),
                 (unsigned)(bdfid & 3));
    }

    return 0;
#else
    return -1;
#endif /* SONAR_AMD_GPU */
}

int amdml_device_get_card_state(uint32_t device, struct amdml_card_state_t* infobuf) {
#ifdef SONAR_AMD_GPU
    load_rsmi();
    if (num_gpus == -1) {
        /*printf("loaded\n");*/
        return -1;
    }
    if (device >= (uint32_t)num_gpus) {
        /*printf("range\n");*/
        return -1;
    }
    memset(infobuf, 0, sizeof(*infobuf));

    int64_t speed = 0;
    if (xrsmi_dev_fan_speed_get(device, 0, &speed) == 0) {
        infobuf->fan_speed_pct = (float)speed / (float)RSMI_MAX_FAN_SPEED * 100;
    }

    xrsmi_dev_memory_usage_get(device, RSMI_MEM_TYPE_VRAM, &infobuf->mem_used);

    uint64_t power;
    if (xrsmi_dev_current_socket_power_get(device, &power) == 0) {
        infobuf->power = (unsigned)(power / 1000);
    }
    if (xrsmi_dev_power_cap_get(device, 0, &power) == 0) {
        infobuf->power_limit = (unsigned)(power / 1000);
    }

    rsmi_frequencies_t freqs;
    if (xrsmi_dev_gpu_clk_freq_get(device, RSMI_CLK_TYPE_SYS, &freqs) == 0) {
        infobuf->ce_clock = freqs.frequency[freqs.current] / 1000000;
    }
    if (xrsmi_dev_gpu_clk_freq_get(device, RSMI_CLK_TYPE_MEM, &freqs) == 0) {
        infobuf->mem_clock = freqs.frequency[freqs.current] / 1000000;
    }

    rsmi_dev_perf_level_t perfinfo;
    if (xrsmi_dev_perf_level_get(device, &perfinfo) == 0) {
        if (perfinfo == RSMI_DEV_PERF_LEVEL_UNKNOWN) {
            infobuf->perf_level = -1;
        } else {
            infobuf->perf_level = (int)perfinfo;
        }
    }

    /* There are many options.  Here I'm guessing "EDGE" is most relevant and the metric "CURRENT"
       is most compatible with the NVIDIA readings, but there are many more. */
    int64_t temp;
    if (xrsmi_dev_temp_metric_get(device, RSMI_TEMP_TYPE_EDGE, RSMI_TEMP_CURRENT, &temp) == 0) {
        infobuf->temp = (unsigned)(temp / 1000);
    }

    uint32_t busy;
    if (xrsmi_dev_busy_percent_get(device, &busy) == 0) {
        infobuf->gpu_util = (float)busy;
    }
    /* I'm not seeing memory_busy returning anything on my test card.  It can be computed from
       memory total and memory use instead, and for those we do have data. */
    if (xrsmi_dev_memory_busy_percent_get(device, &busy) == 0) {
        infobuf->mem_util = (float)busy;
    }

#if 0
    /* This API is not supported on the test card I have.  rocm_smi will print this as additional
       detail if available but primarily reports the figures retrieved above. */
    rsmi_utilization_counter_t utils[2];
    memset(utils, 0, sizeof(utils));
    utils[0].type = RSMI_COARSE_GRAIN_GFX_ACTIVITY;
    utils[1].type = RSMI_COARSE_GRAIN_MEM_ACTIVITY;
    uint64_t timestamp;
    rsmi_status_t r;
    if ((r = xrsmi_utilization_count_get(device, utils, 2, &timestamp)) == 0) {
        printf(" gfx=%lld mem=%lld\n",
               (unsigned long long)utils[0].value, (unsigned long long)utils[1].value);
    }
#endif

    return 0;
#else
    return -1;
#endif /* SONAR_AMD_GPU */
}

#ifdef SONAR_AMD_GPU
static struct amdml_gpu_process_t* infos;  /* NULL for no info yet */
static uint32_t info_count;
#endif /* SONAR_AMD_GPU */

int amdml_device_probe_processes(uint32_t* count) {
#ifdef SONAR_AMD_GPU
    load_rsmi();
    if (num_gpus == -1) {
        /*printf("loaded\n");*/
        return -1;
    }
    if (infos != NULL) {
        return -1;
    }

    rsmi_process_info_t* procs = NULL;
    uint64_t *card_sizes = NULL;

    uint32_t numprocs = 0;
    if (xrsmi_compute_process_info_get(NULL, &numprocs) != 0) {
        goto bail;
    }
    numprocs *= 2;              /* Headroom */
    procs = calloc(numprocs, sizeof(rsmi_process_info_t));
    if (procs == NULL) {
        goto bail;
    }
    if (xrsmi_compute_process_info_get(procs, &numprocs) != 0) {
        goto bail;
    }

    infos = calloc(numprocs, sizeof(struct amdml_gpu_process_t));
    if (infos == NULL) {
        goto bail;
    }

    card_sizes = calloc(num_gpus, sizeof(uint64_t));
    if (card_sizes == NULL) {
        goto bail;
    }
    for ( int d = 0 ; d < num_gpus ; d++ ) {
        xrsmi_dev_memory_total_get(0, RSMI_MEM_TYPE_VRAM, &card_sizes[d]);
    }

    /* p looks at procs, i looks at infos, the latter may trail the former if we fail to get info */
    unsigned p = 0, i = 0;
    for ( p=0 ; p < numprocs ; p++ ) {
        infos[i].pid = procs[p].process_id;

        /* For whatever reason, mem_util and gpu_util are always zero in this table (at least on the
           card I have), all we get is the pid.  We have to do a secondary lookup by pid to get that
           information.  This is dumb. */
        if (xrsmi_compute_process_info_by_pid_get(procs[p].process_id, &procs[p]) == 0) {
            infos[i].gpu_util = procs[p].cu_occupancy; /* across all cards */
            infos[i].mem_size = procs[p].vram_usage;   /* across all cards */
        }

        /* Then probe the cards that the pid is using */
        uint32_t numcards = 0;
        if (xrsmi_compute_process_gpus_get(procs[p].process_id, NULL, &numcards) != 0) {
            continue;
        }
        numcards *= 2;          /* Headroom */
        uint32_t* cards = calloc(numcards, sizeof(uint32_t));
        if (cards == NULL) {
            continue;
        }
        if (xrsmi_compute_process_gpus_get(procs[p].process_id, cards, &numcards) != 0) {
            continue;
        }
        if (numcards == 0) {
            // This happens on idle cards, for whatever reason.
            continue;
        }

        uint64_t sum_card_sizes = 0;
        for (uint32_t c = 0 ; c < numcards ; c++ ) {
            if (cards[c] <= 31) {
                infos[i].cards |= (1 << cards[c]);
            }
            sum_card_sizes += card_sizes[cards[c]];
        }
        free(cards);
        if (sum_card_sizes > 0) {
            infos[i].mem_util = (100 * procs[p].vram_usage) / sum_card_sizes;
        }

        i++;
    }
    free(procs);
    free(card_sizes);
    info_count = i;
    *count = i;
    return 0;

bail:
    free(procs);
    free(infos);
    free(card_sizes);
    infos = NULL;
    info_count = 0;
    *count = 0;
    return -1;
#else
    return -1;
#endif /* SONAR_AMD_GPU */
}

int amdml_get_process(uint32_t index, struct amdml_gpu_process_t* infobuf) {
#ifdef SONAR_AMD_GPU
    if (infos == NULL) {
        return -1;
    }
    if (index >= info_count) {
        return -1;
    }

    memcpy(infobuf, infos+index, sizeof(struct amdml_gpu_process_t));
    return 0;
#else
    return -1;
#endif /* SONAR_AMD_GPU */
}

void amdml_free_processes() {
#ifdef SONAR_AMD_GPU
    if (infos != NULL) {
        free(infos);
        infos = NULL;
    }
#endif /* SONAR_AMD_GPU */
}
