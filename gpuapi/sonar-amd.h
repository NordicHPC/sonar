/* Static-linkable API to the dynamically-loaded AMD SMI library.

   This API is called from Rust.  Data structures and signatures must be reflected exactly on the
   Rust side.  See ../src/amd_smi.rs.

   Buffer sizes are a bit ad-hoc (and are partly inherited from the NVIDIA code).

   Cards are identified by a device index i s.t. 0 <= i < device_count, ie, the range is dense.

   Functions uniformly return 0 for success (sometimes even when some data where not obtainable but
   the result makes sense) and -1 for failure.

   This library has internal global state and is not thread-safe. */

#ifndef sonar_amd_h_included
#define sonar_amd_h_included

#include <inttypes.h>

/* Get the number of devices. */
int amdml_device_get_count(uint32_t* count);

struct amdml_card_info_t {
    char bus_addr[80];          /* PCI busId extended bdf form, maybe other fabrics later */
    char model[256];            /* dev_name */
    char driver[64];            /* version_str */
    char firmware[32];          /* dev_firmware_version FW_BLOCK_CE, "ce=<version>", may change! */
    char uuid[96];              /* some identifying string, if available; otherwise blank */
    uint64_t totalmem;          /* dev_memory_total, bytes */
    unsigned power_limit;       /* dev_power_cap, mW */
    unsigned min_power_limit;   /* dev_power_cap_range.min, mW */
    unsigned max_power_limit;   /* dev_power_cap.range.max, mW */
    unsigned min_ce_clock;      /* dev_gpu_clk_freq CLK_TYPE_SYS min, MHz */
    unsigned max_ce_clock;      /* dev_gpu_clk_freq CLK_TYPE_SYS max, MHz */
    unsigned min_mem_clock;     /* dev_gpu_clk_freq CLK_TYPE_MEM min, MHz */
    unsigned max_mem_clock;     /* dev_gpu_clk_freq CLK_TYPE_MEM max, MHz */
};

/* Clear the infobuf and fill it with available information. */
int amdml_device_get_card_info(uint32_t device_index, struct amdml_card_info_t* infobuf);

struct amdml_card_state_t {
    float fan_speed_pct;        /* current speed, percent of max */
    int perf_level;             /* -1 for unknown, otherwise a DEV_PERF_LEVEL_X integer */
    uint64_t mem_used;          /* bytes */
    float gpu_util;             /* percent */
    float mem_util;             /* percent */
    unsigned temp;              /* degrees C */
    unsigned power;             /* current power draw - mW */
    unsigned power_limit;       /* current power limit - mW */
    unsigned ce_clock;          /* average over 1s in Mhz */
    unsigned mem_clock;         /* average over 1s in Mhz */
};

/* Clear the infobuf and fill it with available information. */
int amdml_device_get_card_state(uint32_t device_index, struct amdml_card_state_t* infobuf);

/* Probe the card's process tables and save the information in an internal data structure, returning
   the number of processes.  On success, the data structure is always allocated even if count = 0,
   and the data structure must later be freed with amdml_free_processes(). */
int amdml_device_probe_processes(uint32_t* count);

struct amdml_gpu_process_t {
    uint32_t pid;               /* Linux process ID */
    uint32_t cards;             /* bitmap of indices of cards used by this process */
    uint32_t gpu_util;          /* percent utilization across all cards for the process */
    uint32_t mem_util;          /* percent utilization across all cards for the process */
    uint64_t mem_size;          /* bytes used across all cards for the process */
};

/* Get information for the index'th process from the internal buffers and store it into *infobuf.
   This will fail if the index is out of bounds. */
int amdml_get_process(uint32_t process_index, struct amdml_gpu_process_t* infobuf);

/* Free any internal data structures. */
void amdml_free_processes();

#endif /* sonar_amd_h_included */
