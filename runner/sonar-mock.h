#ifndef sonar_mock_h_included
#define sonar_mock_h_included

#include "protocol.h"

result_t test_exe_for_pids(const long* pids, size_t npids, int request_fd, int response_fd,
    void (*sent_hook)(), void (*pid_callback)(uint32_t, const char*));

#endif
