/* sonar-daemon-runner runs (normally as root), forks off a non-privileged `sonar daemon`, and sets
 * itself up as a server that will respond to very limited requests from the sonar daemon for
 * information that is only available to root.
 *
 * Usage:
 *
 *	sonar-daemon-runner sonar-path config-path user-name group-name
 *
 * where sonar-path is the full path to the Sonar executable, config-path is the full path to the
 * Sonar daemon's config file, and user-name and group-name names the user and group under which
 * Sonar should be running.
 *
 * (If run as non-root, does not do anything interesting with privileges, it just runs Sonar as a
 * child process.  Useful mainly for some testing.)
 *
 * Operations currently implemented by the server (see protocol.h):
 *
 * - Terminate (the child can ask the parent to exit with an exit code)
 * - Read /proc/pid/exe for one or more pids
 *
 * As this runs as root, it needs to be as trustworthy and easily audited as we can make it.  Hence
 * it is simple, with straightforward abstractions and limited functionality.  Writing it in C was a
 * tradeoff, but ensures that "all" the code (except libc) is visible to us for inspection.
 *
 * All error logging is currently to stderr.  Under normal circumstances, the runner is run under
 * systemd and systemd will surface the errors via systemctl status.
 *
 *
 * Robustness:
 *
 * - If the child terminates, the server will catch the SIGCHLD and will also terminate (with the
 *   same exit code as the child).
 *
 * - If the server needs to terminate, except by explicit request, it will attempt to terminate
 *   the child by sending it SIGTERM.
 *
 * - Both sides should consider a malformed message from the other side a fatal error, and should
 *   terminate immediately.  There is no error recovery on the channel.
 *
 * - The main risk is that the server does not respond to Sonar in a timely manner, thus hanging
 *   Sonar.  Sonar can in principle handle this with a watchdog timer.
 *
 *
 * Implementation:
 *
 * Communication is over a pipe: The subprocess will send requests and this program will respond
 * with an answer.  The protocol is simple question-answer and is documented in protocol.h.  The
 * protocol may change; do not upgrade this server independently of the Sonar code in
 * ../src/privileged.rs.
 */

#include <errno.h>
#include <grp.h>
#include <inttypes.h>
#include <linux/limits.h>
#include <pwd.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>
#include <unistd.h>

#include "protocol.h"

#ifndef PATH_MAX
#  define PATH_MAX 4096
#endif

void sonar(const char* path, const char* config, const char* user, const char* group,
    int request_fd, int response_fd);
result_t server(int input, int output);
void get_exe(uint32_t pid, char buf[PATH_MAX]);
void sigchld(int);

int main(int argc, char** argv) {
    if (argc != 5) {
        fprintf(stderr, "Usage: %s sonar-path config-path user-name group-name\n", argv[0]);
        return 1;
    }
    int down[2];
    if (pipe(down) != 0) {
        perror("pipe");
        return 1;
    }
    int up[2];
    if (pipe(up) != 0) {
        perror("pipe");
        return 1;
    }
    struct sigaction act;
    memset(&act, 0, sizeof(act));
    act.sa_handler = sigchld;
    /* TODO: SA_NOCLDWAIT is really wrong b/c we want to wait on the child and then exit with its
     * exit code.
     */
    act.sa_flags = SA_NOCLDSTOP | SA_NOCLDWAIT;
    if (sigaction(SIGCHLD, &act, NULL) != 0) {
        perror("signal");
        return 1;
    }

    pid_t pid = fork();
    switch (pid) {
    case -1:
        perror("fork");
        return 1;
    case 0:
        /* child */
        close(down[1]);
        close(up[0]);
        sonar(argv[1], argv[2], argv[3], argv[4], up[1], down[0]);
        return 1;
    default: {
        /* parent */
        close(down[0]);
        close(up[1]);
        result_t r = server(up[0], down[1]);
        if (r != OK) {
            kill(pid, SIGTERM);
        }
        return r != OK;
    }
    }
}

void sigchld(int s) {
    int n = write(2, "Sonar-runner exiting because child did\n", 40);
    (void)n;
    /* TODO: Exit code 1 is not right, we want to wait on the child and then exit with its exit
     * code.
     */
    _exit(1);
}

result_t server(int input, int output) {
    result_t r;
    inbound_t inbound;
    outbound_t outbound;
    init_inbound(&inbound);
    init_outbound(&outbound);
    for (;;) {
        destroy_inbound(&inbound);
        destroy_outbound(&outbound);
        if ((r = recv_message(input, &inbound)) != OK) {
            goto Done;
        }
        uint8_t op;
        if ((r = decode_byte(&inbound, &op)) != OK) {
            goto Done;
        }
        switch (op) {
        case REQ_EXIT: {
            fprintf(stderr, "Sonar-runner exiting by child request\n");
            uint8_t xcode;
            if ((r = decode_byte(&inbound, &xcode)) != OK) {
                goto Done;
            }
            destroy_inbound(&inbound);
            destroy_outbound(&outbound);
            exit(xcode);
        }
        case REQ_EXE_FOR_PIDS: {
            uint32_t nelem;
            if ((r = decode_int(&inbound, &nelem)) != OK) {
                goto Done;
            }
            if ((r = encode_byte(&outbound, op)) != OK) {
                goto Done;
            }
            if ((r = encode_int(&outbound, nelem)) != OK) {
                goto Done;
            }
            for (uint32_t i = 0; i < nelem; i++) {
                uint32_t pid;
                static char exebuf[PATH_MAX];
                if ((r = decode_int(&inbound, &pid)) != OK) {
                    goto Done;
                }
                get_exe(pid, exebuf);
                if ((r = encode_int(&outbound, pid)) != OK) {
                    goto Done;
                }
                if ((r = encode_string(&outbound, exebuf)) != OK) {
                    goto Done;
                }
            }
            if ((r = send_message(output, &outbound)) != OK) {
                goto Done;
            }
            continue;
        }
        default:
            fprintf(stderr, "Unknown operation from child: %d\n", op);
            continue;
        }
    }
Done:
    destroy_inbound(&inbound);
    destroy_outbound(&outbound);
    return r;
}

/* Given a pid, try to get /proc/pid/exe, otherwise return empty string */
void get_exe(uint32_t pid, char buf[PATH_MAX]) {
#ifdef LOGGING
    printf("get_exe %d\n", pid);
#endif
    char path[128];
    snprintf(path, sizeof(path), "/proc/%d/exe", pid);
    ssize_t n;
    if ((n = readlink(path, buf, PATH_MAX)) == -1) {
        /* Failure is common, so just clear out the string */
        n = 0;
    }
    buf[n < PATH_MAX ? n : PATH_MAX - 1] = 0;
}

/* Drop privileges (if running as root) and run the Sonar subprocess with appropriate arguments.  On
 * success, this does not return, but if it does return then the child should exit immediately with
 * an error code.
 */
void sonar(const char* path, const char* config, const char* user, const char* group,
    int request_fd, int response_fd) {
    if (getuid() == 0) {
        struct passwd pw, *presult;
        char buf[1024];
        int e; /* errno */
        if ((e = getpwnam_r(user, &pw, buf, sizeof(buf), &presult)) != 0) {
            fprintf(stderr, "getpwnam_r failed: %d\n", e);
            return;
        }
        if (presult == NULL) {
            fprintf(stderr, "no such user: %s\n", user);
            return;
        }
        uid_t new_uid = pw.pw_uid;
        struct group gr, *gresult;
        if ((e = getgrnam_r(group, &gr, buf, sizeof(buf), &gresult)) != 0) {
            fprintf(stderr, "getgrnam_r failed: %d\n", e);
            return;
        }
        if (gresult == NULL) {
            fprintf(stderr, "no such group: %s\n", group);
            return;
        }
        gid_t new_gid = gr.gr_gid;
        if (setgid(new_gid) != 0) {
            perror("setgid");
            return;
        }
        if (setuid(new_uid) != 0) {
            perror("setuid");
            return;
        }
    } else {
        fprintf(stderr, "Not running as root, not dropping privileges\n");
    }
#ifdef LOGGING
    /* Probably this wants to be "daemon" first... */
    printf("Running %s daemon --request-fd %d --response-fd %d %s\n", path, request_fd, response_fd,
        config);
#endif
    char req[20], resp[20];
    sprintf(req, "%d", request_fd);
    sprintf(resp, "%d", response_fd);
    execl(path, path, "daemon", "--request-fd", req, "--response-fd", resp, config, (char*)NULL);
    perror("exec");
}
