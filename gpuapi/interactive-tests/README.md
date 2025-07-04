Here are programs that can be used to exercise the GPUs so that we can
get interesting data from the various SMI programs or from Sonar's GPU
shells (in the parent directory).

For example, running sycl-mmul in the background on a node with an
Intel XPU, one can run ../xpu-shell -proc and should see the MMUL
process running one one of the accelerators on the node, with
non-trivial memory allocation, or ../xpu-shell -state to see that a
card is busy.

The .cpp files in this directory have instructions for how to load and
use toolchains appropriately, some of this is also encoded in
Makefile.  And also instructions for how to schedule on different
devices and so on.
