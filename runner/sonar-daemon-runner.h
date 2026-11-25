#ifndef sonar_daemon_runner_h_included
#define sonar_daemon_runner_h_included

#include <linux/limits.h> /* PATH_MAX */
#include <signal.h>       /* siginfo_t */
#include <stdint.h>

#ifndef PATH_MAX
#  define PATH_MAX 4096
#endif

int sonar_daemon_runner(const char* sonar_path, const char* config_file, const char* user_name,
    const char* group_name, void (*sigchld_sigaction)(int, siginfo_t*, void*));

/* In a var_pat, p-prefix points to malloc'd storage, len is strlen(p). */
struct var_pat {
    int prefix;
    int suffix;
    char* p;
    size_t len;
};

/* In an env_var, 'name' points to malloc'd storage, name and val are both 0-terminated within that
   storage, freeing the name also frees the value. */
struct env_var {
    char* name;
    char* value;
};

void get_exe(uint32_t pid, char buf[PATH_MAX]);

int get_env_vars(
    uint32_t pid, struct var_pat* pats, uint32_t npats, struct env_var** result, uint32_t* nresult);

#endif /* sonar_daemon_runner_h_included */
