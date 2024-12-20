/* Static-linkable API to the NVIDIA NVML library */

#ifndef sonar_nvidia_h_included
#define sonar_nvidia_h_included

#include <inttypes.h>

int nvml_device_get_count(uint32_t* count);

/* The buffer sizes are mostly mandated by the underlying NVML API */
/* Some of the others are conservative too */
/* CUDA Version is only one possible interpretation of "firmware", the CUDA compute capability
   version could be another. */

/* This structure must be reflected exactly on the Rust side */
struct nvml_card_info {
    char bus_addr[80];          /* PCI busId */
    char model[96];
    char architecture[32];
    char driver[80];            /* Same for all cards on a node */
    char firmware[32];          /* CUDA Version */
    char uuid[96];
    uint64_t totalmem;          /* Bytes */
    unsigned power_limit;       /* Milliwatts */
    unsigned min_power_limit;   /* Milliwatts */
    unsigned max_power_limit;   /* Milliwatts */
    unsigned max_ce_clock;      /* MHz */
    unsigned max_mem_clock;     /* MHz */
};

/* Clear the infobuf and fill it with available information.  Return 0 on success, -1 on any kind of
   error. */
int nvml_device_get_card_info(uint32_t device, struct nvml_card_info* infobuf);

struct nvml_card_state {
    unsigned fan_speed;
    char compute_mode[32];
    char perf_state[8];         /* May be "Unknown" or P<n> for lowish n */
    uint64_t mem_reserved;
    uint64_t mem_used;
    float gpu_util;
    float mem_util;
    unsigned temp;
    unsigned power;
    unsigned power_limit;
    unsigned ce_clock;
    unsigned mem_clock;
};

/* Clear the infobuf and fill it with available information. Return 0 on success, -1 on any kind of
   error. */
int nvml_device_get_card_state(uint32_t device, struct nvml_card_state* infobuf);

struct nvml_gpu_process {
    uint32_t pid;
    uint32_t mem_util;          /* percent */
    uint32_t gpu_util;          /* percent */
    uint64_t mem_size;          /* KB */
};

/* Probe the card's process tables and save the information in an internal data structure.  Return 0
   on success along with a count of processes, -1 on failure.  If 0 is returned then a data
   structure is always allocated even if count = 0, and this data structure must be freed with
   nvml_free_processes().
 */
int nvml_device_probe_processes(uint32_t device, uint32_t* count);

/* Get information for the given process from the internal buffers and store it into *infobuf.  Return 0
   on success, -1 on failure (eg, out of bounds - but this indicates a program bug, not a system failure).
*/
int nvml_get_process(uint32_t index, struct nvml_gpu_process* infobuf);

/* Free any internal data structures. */
void nvml_free_processes();

#endif /* sonar_nvidia_h_included */
