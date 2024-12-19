/* Static-linkable API to the NVIDIA NVML library */

#ifndef sonar_nvidia_h_included
#define sonar_nvidia_h_included

#include <inttypes.h>

int nvml_open();
int nvml_close();

int nvml_device_get_count(uint32_t* count);

/* The buffer sizes are mostly mandated by the underlying NVML API */
/* This structure must be reflected exactly on the Rust side */
struct nvml_card_info {
    char bus_addr[80];
    char model[96];
    char architecture[32];
    char driver[80];            /* Same for all cards on a node */
    char firmware[80];
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
    char perf_state[8];
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

#endif /* sonar_nvidia_h_included */
