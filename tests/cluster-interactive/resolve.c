#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
#include <string.h>

int main(int argc, char** argv) {
    if (argc != 2) {
        abort();
    }
    char* node = argv[1];
    struct addrinfo *res;
    struct addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = AI_CANONNAME;
    int r;
    if ((r = getaddrinfo(node, NULL, &hints, &res)) != 0) {
        puts(gai_strerror(r));
        exit(1);
    }

    for ( struct addrinfo *p = res; p != NULL ; p = p->ai_next ) {
        if (p->ai_canonname != NULL) {
            printf("canon: %s\n", p->ai_canonname);
        }
        char host[1024];
        if (getnameinfo(p->ai_addr, p->ai_addrlen, host, sizeof(host), NULL, 0, NI_NAMEREQD) == 0) {
            puts(host);
        }
    }

    freeaddrinfo(res);
    return 0;
}
