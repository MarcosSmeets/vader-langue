/* Atribuição: o caminho HTTP real (accept/respond) vaza por request? */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <malloc.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>

extern void *vader_http_listen(long);
extern int vader_http_accept(void *);
extern const char *vader_http_path(void *);
extern void vader_http_respond(void *, long, const char *, const char *);
extern void *vader_json_object(void);
extern void *vader_json_set_str(void *, const char *, const char *);
extern const char *vader_json_encode(void *);

int main(void) {
    int N = 20000, warm = 200;
    void *s = vader_http_listen(18091);
    pid_t pid = fork();
    if (pid == 0) {
        usleep(300000);
        for (int i = 0; i < N; i++) {
            int fd = socket(AF_INET, SOCK_STREAM, 0);
            struct sockaddr_in a;
            memset(&a, 0, sizeof a);
            a.sin_family = AF_INET;
            a.sin_port = htons(18091);
            a.sin_addr.s_addr = inet_addr("127.0.0.1");
            if (connect(fd, (void *)&a, sizeof a) == 0) {
                write(fd, "GET /x HTTP/1.1\r\nHost: x\r\n\r\n", 28);
                char b[512];
                read(fd, b, sizeof b);
            }
            close(fd);
        }
        _exit(0);
    }
    struct mallinfo2 m0 = {0}, m1;
    for (int i = 0; i < N; i++) {
        if (!vader_http_accept(s)) break;
        if (i == warm) m0 = mallinfo2();
        void *o = vader_json_object();
        vader_json_set_str(o, "path", vader_http_path(s));
        vader_json_set_str(o, "msg", "ola");
        vader_http_respond(s, 200, "application/json", vader_json_encode(o));
    }
    m1 = mallinfo2();
    long d = (long)m1.uordblks - (long)m0.uordblks;
    printf("server uordblks delta apos %d reqs: %ld bytes (%.3f/req)\n", N - warm, d,
           d / (double)(N - warm));
    return 0;
}
