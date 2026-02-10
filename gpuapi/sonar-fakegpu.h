/* Static-linkable API to a stub GPU library.

   This API is called from Rust.  Data structures and signatures must be reflected exactly on the
   Rust side.  See ../src/gpu/fakegpu_smi.rs.

   Cards are identified by a device index i s.t. 0 <= i < device_count, ie, the range is dense.

   Functions uniformly return 0 for success (sometimes even when some data where not obtainable but
   the result makes sense) and -1 for failure.

   This library has internal global state and is not thread-safe. */

#ifndef sonar_fakegpu_h_included
#define sonar_fakegpu_h_included

#include <inttypes.h>

/* Get the number of devices. */
int fakegpu_device_get_count(uint32_t* count);

struct fakegpu_card_info_t {
    char bus_addr[256];       /* PCI busId extended bdf form, maybe other fabrics later */
    char model[256];          /* Manufacturer's model name, human-readable */
    char driver[256];         /* Same for all cards on a node? */
    char firmware[256];       /* Onboard firmware "name @ version" */
    char uuid[256];           /* some identifying string, if available; otherwise blank */
    uint64_t totalmem;        /* bytes */
    unsigned max_ce_clock;    /* core clock rate MHz */
    unsigned max_power_limit; /* sustained, W */
};

/* Clear the infobuf and fill it with available information for the device. */
int fakegpu_device_get_card_info(uint32_t device_index, struct fakegpu_card_info_t* infobuf);

struct fakegpu_card_state_t {
    /* The underlying stats API is fairly rich, we could do better than this */
    float gpu_util;    /* utilizationRates gpu; percent */
    float mem_util;    /* utilizationRates memory; percent */
    uint64_t mem_used; /* memoryInfo used; bytes */
    unsigned temp;     /* temperature, degrees C */
    unsigned power;    /* powerUsage, mW */
    unsigned ce_clock; /* clockInfo CLOCK_SM, MHz */
};

/* Clear the infobuf and fill it with available information. */
int fakegpu_device_get_card_state(uint32_t device_index, struct fakegpu_card_state_t* infobuf);

/* Probe the card's process tables and save the information in an internal data structure, returning
   the number of processes.  On success, the data structure is always allocated even if count = 0,
   and the data structure must be freed with nvml_free_processes(). */
int fakegpu_device_probe_processes(uint32_t device_index, uint32_t* count);

struct fakegpu_gpu_process_t {
    uint32_t pid;      /* Linux process ID */
    uint32_t mem_util; /* percent */
    uint32_t gpu_util; /* percent */
    uint64_t mem_size; /* KB */
};

/* Get information for the given process from the internal buffers and store it into *infobuf.  This
   will fail if the index is out of bounds. */
int fakegpu_get_process(uint32_t process_index, struct fakegpu_gpu_process_t* infobuf);

/* Free any internal data structures. */
void fakegpu_free_processes();

#endif /* sonar_fakegpu_h_included */
