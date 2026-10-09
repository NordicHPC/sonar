#ifndef sonar_daemon_runner_h_included
#define sonar_daemon_runner_h_included

#include <linux/limits.h> /* PATH_MAX */
#include <stdint.h>

#ifndef PATH_MAX
#  define PATH_MAX 4096
#endif

int sonar_daemon_runner(const char* sonar_path, const char* config_file, const char* user_name,
    const char* group_name, void (*sigchld_handler)(int));
void get_exe(uint32_t pid, char buf[PATH_MAX]);

#endif /* sonar_daemon_runner_h_included */
