/* sonar-daemon-runner runs as root, forks off a non-privileged `sonar daemon`, and sets itself up
 * as a server that will respond to very limited requests from the sonar daemon for information that
 * is only available to root.
 *
 * Usage (as root):
 *
 *	sonar-daemon-runner sonar-path config-path user-name group-name
 *
 * where sonar-path is the full path to the Sonar executable, config-path is the full path to the
 * Sonar daemon's config file, and user-name and group-name names the user and group under which
 * Sonar should be running.
 *
 * Communication is over a pipe: The subprocess will send requests and this program will respond
 * with an answer.  The protocol is simple question-answer and is documented in protocol.h.  The
 * protocol may change; do not upgrade this server independently of the Sonar subprocess (or the
 * test process in subproc.c).
 *
 * If the child terminates, the server will also terminate (with the same exit code as the child).
 *
 * The child can optionally send a message asking the server to terminate with a specific exit code.
 *
 * All logging is currently to stderr and is done as close to the error site as possible.  Code that
 * just propagates an error may assume that the error has been logged at the originating site.
 * Under normal circumstances, the runner is run under systemd and systemd will surface the errors
 * via systemctl status.
 *
 * Notes:
 *
 * This needs to be as trustworthy as possible, so it is simple and uses straightforward
 * abstractions to maintain that simplicity.
 *
 * Error propagation: all functions return OK for success and ERR_SOMETHING on error.  The error
 * code should be propagated whenever possible.
 *
 * Robustness:
 *
 * Pretty every error is considered fatal and will cause it to kill the child and exit.  If the
 * child exits, this will also exit.
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
result_t get_exe(uint32_t pid, char buf[PATH_MAX]);
void sigchld(int);

int main(int argc, char** argv) {
    if (argc != 5) {
        fprintf(stderr, "Usage: %s sonar-path config-path user-name group-name\n", argv[0]);
        return 1;
    }
    int down[2];
    if (pipe(down) != 0) {
        perror("pipe2");
        return 1;
    }
    int up[2];
    if (pipe(up) != 0) {
        perror("pipe2");
        return 1;
    }
    struct sigaction act;
    memset(&act, 0, sizeof(act));
    act.sa_handler = sigchld;
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
        kill(pid, SIGTERM);
        return r != OK;
    }
    }
}

void sigchld(int) {
#ifdef LOGGING
    int n = write(1, "RUNNER: SIGCHLD\n", 16);
    (void)n;
#endif
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
        case REQ_EXIT:
            fprintf(stderr, "Sonar-runner exiting by child request\n");
            r = OK;
            goto Done;
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
                if (get_exe(pid, exebuf) != OK) {
                    /* Soft error: just send empty string */
                    *exebuf = 0;
                }
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

/* Given a pid, try to get /proc/pid/exe. */
result_t get_exe(uint32_t pid, char buf[PATH_MAX]) {
#ifdef LOGGING
    printf("get_exe %d\n", pid);
#endif
    char path[128];
    snprintf(path, sizeof(path), "/proc/%d/exe", pid);
    ssize_t n;
    if ((n = readlink(path, buf, PATH_MAX)) == -1) {
        fprintf(stderr, "Readlink failed for %s\n", path);
        return ERR_IO;
    }
    buf[n < PATH_MAX ? n : PATH_MAX - 1] = 0;
    return OK;
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
    // Probably this wants to be "daemon" first...
    printf("Running %s daemon --request-fd %d --response-fd %d %s\n", path, request_fd, response_fd,
        config);
#endif
    char req[20], resp[20];
    sprintf(req, "%d", request_fd);
    sprintf(resp, "%d", response_fd);
    execl(path, path, "daemon", "--request-fd", req, "--response-fd", resp, config, (char*)NULL);
    perror("exec");
}
