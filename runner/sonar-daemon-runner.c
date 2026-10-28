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
 * - If the server needs to terminate, except by explicit request from the child, it will attempt to
 *   terminate the child by sending it SIGTERM.
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
#include <pwd.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <unistd.h>

#include "protocol.h"
#include "sonar-daemon-runner.h"

static result_t server(int input, int output);
static void sonar(const char* path, const char* config, const char* user, const char* group,
    int request_fd, int response_fd);

#ifndef SELFTEST
static void sigchld(int, siginfo_t*, void*);

int main(int argc, char** argv) {
    if (argc != 5) {
        fprintf(stderr, "Usage: %s sonar-path config-path user-name group-name\n", argv[0]);
        return 1;
    }
    return sonar_daemon_runner(argv[1], argv[2], argv[3], argv[4], sigchld);
}

static void sigchld(int sig, siginfo_t* si, void* data) {
    int n = write(2, "Sonar-runner exiting because child did\n", 40);
    (void)n;
    if (si->si_code == CLD_EXITED) {
        _exit(si->si_status);
    }
    /* Killed or dumped - SA_NOCLDSTOP ensures no STOPPED/CONTINUED signal,
       and we're not tracing the child. */
    _exit(1);
}
#endif

/* Given a pid, try to get /proc/pid/exe, otherwise return empty string */
void get_exe(uint32_t pid, char buf[PATH_MAX]) {
#if defined(LOGGING) && defined(SELFTEST)
    fprintf(stderr, "get_exe %" PRIu32 "\n", pid);
    fflush(stderr);
#endif
    char path[128];
    snprintf(path, sizeof(path), "/proc/%" PRIu32 "/exe", pid);
    ssize_t n;
    if ((n = readlink(path, buf, PATH_MAX)) == -1) {
        /* Failure is common, so just clear out the string */
        n = 0;
    }
    buf[n < PATH_MAX ? n : PATH_MAX - 1] = 0;
}

result_t req_exe_for_pids(inbound_t* inbound, outbound_t* outbound) {
    result_t r;
    uint32_t nelem;
    if ((r = decode_int(inbound, &nelem)) != OK) {
        return r;
    }
    if ((r = encode_byte(outbound, REQ_EXE_FOR_PIDS)) != OK) {
        return r;
    }
    if ((r = encode_int(outbound, nelem)) != OK) {
        return r;
    }
    for (uint32_t i = 0; i < nelem; i++) {
        uint32_t pid;
        static char exebuf[PATH_MAX];
        if ((r = decode_int(inbound, &pid)) != OK) {
            return r;
        }
        get_exe(pid, exebuf);
        if ((r = encode_int(outbound, pid)) != OK) {
            return r;
        }
        if ((r = encode_string(outbound, exebuf)) != OK) {
            return r;
        }
    }
    return OK;
}

int sonar_daemon_runner(const char* sonar_path, const char* config_file, const char* user_name,
    const char* group_name, void (*sigchld_sigaction)(int, siginfo_t*, void*)) {
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

    {
        struct sigaction act;
        act.sa_handler = NULL;
        act.sa_sigaction = NULL;
        act.sa_flags = 0;
        sigemptyset(&act.sa_mask);

        act.sa_sigaction = sigchld_sigaction;
        act.sa_flags = SA_NOCLDSTOP | SA_NOCLDWAIT | SA_SIGINFO;
        if (sigaction(SIGCHLD, &act, NULL) != 0) {
            perror("sigaction for SIGCHLD");
            return 1;
        }
        act.sa_sigaction = NULL;
        act.sa_flags = 0;

        /* We're going to let I/O errors on read/write handle the case where the pipe is closed on
         * the other end.  The Sonar child will inherit this setting for SIGPIPE and will have to
         * install its own handler if it wants something else - but it doesn't, and anyway Rust
         * stdlib ignores SIGPIPE by default, so this is fine.
         */
        act.sa_handler = SIG_IGN;
        if (sigaction(SIGPIPE, &act, NULL) != 0) {
            perror("sigaction for SIGPIPE");
            return 1;
        }
        act.sa_handler = NULL;
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
        sonar(sonar_path, config_file, user_name, group_name, up[1], down[0]);
        return 1;
    default: {
        /* parent */
        close(down[0]);
        close(up[1]);
        result_t r = server(up[0], down[1]);
        if (r != OK) {
            struct sigaction act;
            act.sa_sigaction = NULL;
            act.sa_flags = 0;
            sigemptyset(&act.sa_mask);
            /* SIGCHLD+SIG_IGN will automatically prevent zombiefication of the child */
            act.sa_handler = SIG_IGN;
            sigaction(SIGCHLD, &act, NULL);
            kill(pid, SIGTERM);
        }
        return r != OK;
    }
    }
}

static result_t server(int input, int output) {
    result_t r;
    inbound_t inbound;
    outbound_t outbound;
    init_inbound(&inbound);
    init_outbound(&outbound);
    for (;;) {
        destroy_inbound(&inbound);
        destroy_outbound(&outbound);
        if ((r = recv_message(input, &inbound)) != OK) {
#ifdef LOGGING
            fprintf(stderr, "EOF or error on channel in server, child exited\n");
            fflush(stderr);
#endif
            goto Done;
        }
        uint8_t op;
        if ((r = decode_byte(&inbound, &op)) != OK) {
            goto Done;
        }
        switch (op) {
        case REQ_EXIT: {
#ifdef LOGGING
            fprintf(stderr, "Sonar-runner exiting by child request\n");
            fflush(stderr);
#endif
            uint8_t xcode;
            if ((r = decode_byte(&inbound, &xcode)) != OK) {
                goto Done;
            }
            destroy_inbound(&inbound);
            destroy_outbound(&outbound);
            exit(xcode);
        }
        case REQ_EXE_FOR_PIDS:
            if ((r = req_exe_for_pids(&inbound, &outbound)) != OK) {
                goto Done;
            }
            if ((r = send_message(output, &outbound)) != OK) {
                goto Done;
            }
            continue;
        default:
            fprintf(stderr, "Unknown operation from child: %d\n", op);
            r = ERR_INVALID_OP;
            goto Done;
        }
    }
Done:
    destroy_inbound(&inbound);
    destroy_outbound(&outbound);
    return r;
}

/* Drop privileges (if running as root) and run the Sonar subprocess with appropriate arguments.  On
 * success, this does not return, but if it does return then the child should exit immediately with
 * an error code.
 */
static void sonar(const char* path, const char* config, const char* user, const char* group,
    int request_fd, int response_fd) {
    if (getuid() == 0) {
        struct passwd pw, *presult;
        char buf[1024];
        int e; /* errno */
        if ((e = getpwnam_r(user, &pw, buf, sizeof(buf), &presult)) != 0) {
            perror("getpwnam_r");
            return;
        }
        if (presult == NULL) {
            fprintf(stderr, "no such user: %s\n", user);
            return;
        }
        uid_t new_uid = pw.pw_uid;
        struct group gr, *gresult;
        if ((e = getgrnam_r(group, &gr, buf, sizeof(buf), &gresult)) != 0) {
            perror("getgrnam_r");
            return;
        }
        if (gresult == NULL) {
            fprintf(stderr, "no such group: %s\n", group);
            return;
        }
        gid_t new_gid = gr.gr_gid;
        /* Initgroups removes all supplementary groups of root (including group 0) and adds the
         * supplementary groups of the new user.  If running under systemd this is unnecessary, but
         * if running interactively as root or running setuid-root it's necessary to prevent the
         * child from being able to access files that are group-accessible to root.
         */
        if (initgroups(user, new_gid) != 0) {
            perror("initgroups");
            return;
        }
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
    fprintf(stderr, "Running %s daemon --request-fd %d --response-fd %d %s\n", path, request_fd,
        response_fd, config);
    fflush(stderr);
#endif
    char req[20], resp[20];
    sprintf(req, "%d", request_fd);
    sprintf(resp, "%d", response_fd);
    execl(path, path, "daemon", "--request-fd", req, "--response-fd", resp, config, (char*)NULL);
    perror("exec");
}
