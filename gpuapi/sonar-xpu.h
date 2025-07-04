/* Static-linkable API to the dynamically-loaded Intel XPU SMI library.

   This API is called from Rust.  Data structures and signatures must be reflected exactly on the
   Rust side.  See ../src/gpu/xpu_smi.rs.

   Buffer sizes are a bit ad-hoc (and are partly inherited from the NVIDIA/AMD code).

   Functions uniformly return 0 for success (sometimes even when some data where not obtainable but
   the result makes sense) and -1 for failure.

   This library has internal global state and is not thread-safe. */

#ifndef sonar_xpu_h_included
#define sonar_xpu_h_included

#include <inttypes.h>

/* Get the number of devices. */
int xpu_device_get_count(uint32_t* count);

struct xpu_card_info_t {
    char bus_addr[80];  /* PCI busId extended bdf form, maybe other fabrics later */
    char uuid[96];      /* some identifying string, if available; otherwise blank */
};

/* Clear the infobuf and fill it with available information. */
int xpu_device_get_card_info(uint32_t device, struct xpu_card_info_t* infobuf);

#endif /* sonar_xpu_h_included */
