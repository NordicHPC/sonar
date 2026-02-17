/* Simple test program that can be used on a cluster to probe how name resolution works.  You feed
   it a node name, and it will print out what getaddrinfo and getnameinfo reports about that name.
   Sometimes this is useful when nodes are not properly configured, for example, on fox the
   canonical name of a node is eg c1-10.fox but due to how address info is set up the getnameinfo
   call only returns c1-10.  In turn, this means that the Sonar config file for nodes on that
   cluster may need to have a "domain = .fox" setting in the [cluster] section for node names to be
   transmitted properly. */

#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
#include <string.h>

int main(int argc, char** argv) {
    if (argc != 2) {
        printf("Usage: %s hostname\n", argv[0]);
        return 2;
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
        return 1;
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
