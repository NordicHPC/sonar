/* This is meant to run as root.  It forks off a non-privileged Sonar and sets itself up as a
   partner that will respond to very limited requests for information that is only available to
   root.  Communication is over a pipe: Sonar may send questions and this program will respond
   with an answer.
*/

#include <inttype.h>
#include <unistd.h>

/* An outgoing String is a (say) uint16_t length followed by raw bytes, with no terminator. */
/* Batching would be nice but makes the protocol a little harder? */

enum {
    REQ_INVALID = 0,            /* No payload or response */
    REQ_EXIT = 1,               /* No payload or response */
    REQ_EXE_FOR_PID = 2,        /* Incoming: uint32_t pid; outgoing: String */
    REQ_LAST
};

/* Arguments:
   -user   sonar-user-name
   -group  sonar-group-name
   -sonar  path-to-sonar-executable
   -config path-to-sonar-config-file

   Chief problem:

   We're going to lower privileges and then run `sonar daemon config-file` but we need to coordinate
   the name of the communication channel and we ideally want this channel to be invisible to anyone
   outside the two processes.

   A pipe would be best but then the Sonar subprocess must be able to know the proper FDs and we
   need to guarantee that those FDs are not being used by the Rust runtime.  I'm guessing this will
   be OK and that the FDs can be communicated by env vars, in the worst case.
*/

int main(int argc, char** argv) {
    /* TODO: Check that we're running as root, since otherwise it won't be possible to lower privileges */

    int p[2];
    /* I don't think we need O_DIRECT here */
    if (pipe2(&p, 0) == -1) {
        return 1;
    }
    int input = p[1], output = p[2];

    for (;;) {
        char inbuf[sizeof(uint64)];
        int n = read(input, &inbuf, 1);
        if (n == 0) {
            /* EOF */
            return 1;
        }
        if (n < 0) {
            /* ERROR */
            return 1;
        }
        uint8_t t = (uint8_t)inbuf[0];
        if (t >= REQ_LAST) {
            /* BOGUS */
            continue;
        }
        switch (t) {
        case REQ_INVALID:
            continue;
        case REQ_EXIT:
            return 0;
        case REQ_EXE_FOR_PID:
            /* read pid */
            /* read /proc/pid/exe into big enough buffer */
            /* send length */
            /* send chars, probably sanitized somehow */
            break;
        default:
            continue;
        }
    }
}
