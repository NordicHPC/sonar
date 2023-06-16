// Get info about AMD graphics cards by parsing the output of rocm-smi.
// Something better than this is likely needed.
//
// The output is noisy and it may be partial.
//
//
// Current processes running on GPUs:
//
//   $ rocm-smi --showpidgpus
//
//   ============================= GPUs Indexed by PID ==============================
//   PID 25774 is using 1 DRM device(s):
//   0 
//   ================================================================================
//
// When no processes are running:
//
//   ============================= GPUs Indexed by PID ==============================
//   No KFD PIDs currently running
//   ================================================================================
//
// State machine triggered by "= GPUs Indexed by PID =" and ended by the next line
// with "=====" seems pretty safe.
//
// When multiple devices are in use with multiple processes:
//
//   ============================= GPUs Indexed by PID ==============================
//   PID 28156 is using 1 DRM device(s):
//   1 
//   PID 28154 is using 1 DRM device(s):
//   0 
//   ================================================================================
//
// When one process uses multiple devices:
//
//   ============================= GPUs Indexed by PID ==============================
//   PID 29212 is using 2 DRM device(s):
//   0 1 
//   ================================================================================
//
//
// General stats:
//
//   $ rocm-smi
//
//   ================================= Concise Info =================================
//   GPU  Temp (DieEdge)  AvgPwr  SCLK     MCLK    Fan     Perf  PwrCap  VRAM%  GPU%  
//   0    53.0c           220.0W  1576Mhz  945Mhz  10.98%  auto  220.0W   57%   99%   
//   1    26.0c           3.0W    852Mhz   167Mhz  9.41%   auto  220.0W    0%   0%    
//   ================================================================================
//
// State machine triggered by "= Concise Info =" and terminated by "=====" seems safe.
// We get percentage of memory used but not absolute numbers on this card (according to
// the web, it is not supported).
//
// Unfortunately I've not been able to combine these two yet so we have to run the
// command twice, not a totally happy situation.
//
// It may be sensible to have some error checking here, for example, that the header
// matches something expected.

