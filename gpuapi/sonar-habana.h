/* Static-linkable API to the dynamically-loaded Intel Habana SMI library.

   This API is called from Rust.  Data structures and signatures must be reflected exactly on the
   Rust side.  See ../src/gpu/habana_smi.rs.

   Cards are identified by a device index i s.t. 0 <= i < device_count, ie, the range is dense.

   Functions uniformly return 0 for success (sometimes even when some data where not obtainable but
   the result makes sense) and -1 for failure.

   This library has internal global state and is not thread-safe. */

#ifndef sonar_habana_h_included
#define sonar_habana_h_included

#include <inttypes.h>

/* Get the number of devices. */
int habana_device_get_count(uint32_t* count);

struct habana_card_info_t {
    char bus_addr[256];       /* PCI busId extended bdf form, maybe other fabrics later */
    char model[256];          /* Manufacturer's model name, human-readable */
    char driver[256];         /* Same for all cards on a node? */
    char firmware[256];       /* Onboard firmware */
    char uuid[256];           /* some identifying string, if available; otherwise blank */
    uint64_t totalmem;        /* bytes */
    unsigned max_ce_clock;    /* core clock rate MHz */
    unsigned max_power_limit; /* sustained, W */
};

/* Clear the infobuf and fill it with available information for the device. */
int habana_device_get_card_info(uint32_t device_index, struct habana_card_info_t* infobuf);

#define PERF_STATE_UNKNOWN -1
/* Otherwise a nonnegative integer */

struct habana_card_state_t {
    int perf_state;    /* PERF_STATE_UNKNOWN or n >= 0 */
    float gpu_util;    /* utilizationRates gpu; percent */
    float mem_util;    /* utilizationRates memory; percent */
    uint64_t mem_used; /* memoryInfo used; bytes */
    unsigned temp;     /* temperature, degrees C */
    unsigned power;    /* powerUsage, mW */
    unsigned ce_clock; /* clockInfo CLOCK_SM, MHz */
};

/* Clear the infobuf and fill it with available information. */
int habana_device_get_card_state(uint32_t device_index, struct habana_card_state_t* infobuf);

#endif /* sonar_habana_h_included */
