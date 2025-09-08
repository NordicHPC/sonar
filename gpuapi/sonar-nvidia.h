/* Static-linkable API to the dynamically-loaded NVIDIA NVML library.

   This API is called from Rust.  Data structures and signatures must be reflected exactly on the
   Rust side.  See ../src/nvidia_nvml.rs.

   Most buffer sizes are mandated by the underlying NVML API; some are simply conservative.

   Cards are identified by a device index i s.t. 0 <= i < device_count, ie, the range is dense.

   Functions uniformly return 0 for success (sometimes even when some data where not obtainable but
   the result makes sense) and -1 for failure.

   This library has internal global state and is not thread-safe. */

#ifndef sonar_nvidia_h_included
#define sonar_nvidia_h_included

#include <inttypes.h>

/* Get the number of devices. */
int nvml_device_get_count(uint32_t* count);

/* CUDA Version is only one possible interpretation of "firmware", the CUDA compute capability
   version could be another. */
struct nvml_card_info {
    char bus_addr[80];          /* pci_info busId, maybe other fabrics later */
    char model[96];             /* device name */
    char architecture[32];      /* device architecture or "(unknown)" */
    char driver[80];            /* Same for all cards on a node */
    char firmware[32];          /* CUDA Version */
    char uuid[96];              /* device uuid */
    uint64_t totalmem;          /* memoryInfo total; bytes */
    unsigned power_limit;       /* powerManagementLimit, mW */
    unsigned min_power_limit;   /* powerManagementLimitConstraints min, mW */
    unsigned max_power_limit;   /* powerManagementLimitConstraints max, mW */
    unsigned max_ce_clock;      /* maxClockInfo CLOCK_SM, MHz */
    unsigned max_mem_clock;     /* maxClockInfo CLOCK_MEM, MHz */
};

/* Clear the infobuf and fill it with available information. */
int nvml_device_get_card_info(uint32_t device_index, struct nvml_card_info* infobuf);

#define COMP_MODE_UNKNOWN -1
#define COMP_MODE_DEFAULT 0
#define COMP_MODE_PROHIBITED 1
#define COMP_MODE_EXCLUSIVE_PROCESS 2

#define PERF_STATE_UNKNOWN -1
/* Otherwise a nonnegative integer */

struct nvml_card_state {
    unsigned fan_speed;         /* percent of max, but may go over 100 */
    int compute_mode;           /* COMP_MODE_X, defined above */
    int perf_state;             /* PERF_STATE_UNKNOWN or n >= 0 */
    uint64_t mem_reserved;      /* memoryInfo total - (free + used); bytes */
    uint64_t mem_used;          /* memoryInfo used; bytes */
    float gpu_util;             /* utilizationRates gpu; percent */
    float mem_util;             /* utilizationRates memory; percent */
    unsigned temp;              /* temperature, degrees C */
    unsigned power;             /* powerUsage, mW */
    unsigned power_limit;       /* powerManagementLimit, mW */
    unsigned ce_clock;          /* clockInfo CLOCK_SM, MHz */
    unsigned mem_clock;         /* clockInfo CLOCK_MEM, MHz */
};

/* Clear the infobuf and fill it with available information. */
int nvml_device_get_card_state(uint32_t device_index, struct nvml_card_state* infobuf);

/* Probe the card's process tables and save the information in an internal data structure, returning
   the number of processes.  On success, the data structure is always allocated even if count = 0,
   and the data structure must be freed with nvml_free_processes(). */
int nvml_device_probe_processes(uint32_t device_index, uint32_t* count);

struct nvml_gpu_process {
    uint32_t pid;               /* Linux process ID */
    uint32_t mem_util;          /* percent */
    uint32_t gpu_util;          /* percent */
    uint64_t mem_size;          /* KB */
};

/* Get information for the given process from the internal buffers and store it into *infobuf.  This
   will fail if the index is out of bounds. */
int nvml_get_process(uint32_t process_index, struct nvml_gpu_process* infobuf);

/* Free any internal data structures. */
void nvml_free_processes();

#endif /* sonar_nvidia_h_included */
