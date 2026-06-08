/* Vader runtime HTTP/1.1 — server (accept loop) + client (get/post).
 * Self-contained (sockets), no TLS in v1 (https client comes later).
 * No GC: returned strings leak, in line with the runtime. */
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>
#include <arpa/inet.h>

/* arena allocator (vader_mem.c): the server cycles one arena per request */
extern void *vader_alloc(long n);
extern void *vader_realloc(void *old, long oldn, long newn);
extern char *vader_strdup(const char *s);
extern void *vader_scope(void);
extern void vader_reset(void *arena);
extern void vader_release(void *arena);

static int h_write_all(int fd, const unsigned char *buf, int n) {
    int s = 0;
    while (s < n) {
        int w = write(fd, buf + s, n - s);
        if (w <= 0) return -1;
        s += w;
    }
    return 0;
}

/* ===================== server ============================================ */
typedef struct {
    int lfd; /* listening socket */
    int cfd; /* current connection (-1 = none) */
    char method[16];
    char *path;
    char *body;
    char *headers; /* raw header block (includes the request line) */
    void *req_arena; /* current request's arena (freed on the next accept) */
} HttpServer;

void *vader_http_listen(long port) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return 0;
    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons((unsigned short)port);
    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) return 0;
    if (listen(fd, 16) < 0) return 0;
    HttpServer *s = calloc(1, sizeof(HttpServer));
    s->lfd = fd;
    s->cfd = -1;
    return s;
}

/* accepts a connection and parses the request. 1 = ok, 0 = error. Blocks. */
int vader_http_accept(void *sv) {
    HttpServer *s = sv;
    if (!s) return 0;
    int c = accept(s->lfd, 0, 0);
    if (c < 0) return 0;
    s->cfd = c;
    s->method[0] = 0;

    /* bounded memory: reuses ONE arena per server, resetting it on each request
       (no malloc/free churn). The loop body (parse + JSON + strings) allocates
       here and is discarded on the next accept. */
    if (s->req_arena)
        vader_reset(s->req_arena);
    else
        s->req_arena = vader_scope();

    char buf[65536];
    int n = 0, hdr_end = -1;
    while (n < (int)sizeof(buf) - 1) {
        int r = read(c, buf + n, sizeof(buf) - 1 - n);
        if (r <= 0) break;
        n += r;
        buf[n] = 0;
        char *p = strstr(buf, "\r\n\r\n");
        if (p) {
            hdr_end = (int)(p - buf);
            break;
        }
    }
    if (hdr_end < 0) {
        s->path = vader_strdup("");
        s->body = vader_strdup("");
        s->headers = vader_strdup("");
        return 1;
    }
    char m[16] = {0}, path[2048] = {0};
    sscanf(buf, "%15s %2047s", m, path);
    strncpy(s->method, m, 15);
    s->path = vader_strdup(path);
    s->headers = vader_alloc(hdr_end + 1);
    memcpy(s->headers, buf, hdr_end);
    s->headers[hdr_end] = 0;

    int clen = 0;
    char *cl = strcasestr(buf, "content-length:");
    if (cl)
        clen = atoi(cl + 15);
    int body_start = hdr_end + 4;
    int have = n - body_start;
    char *body = vader_alloc(clen + 1);
    int copy = have < clen ? have : clen;
    if (copy > 0)
        memcpy(body, buf + body_start, copy);
    int got = copy;
    while (got < clen) {
        int r = read(c, body + got, clen - got);
        if (r <= 0) break;
        got += r;
    }
    body[got] = 0;
    s->body = body;
    return 1;
}

const char *vader_http_method(void *sv) {
    HttpServer *s = sv;
    return vader_strdup(s && s->method[0] ? s->method : "");
}
const char *vader_http_path(void *sv) {
    HttpServer *s = sv;
    return vader_strdup(s && s->path ? s->path : "");
}
const char *vader_http_body(void *sv) {
    HttpServer *s = sv;
    return vader_strdup(s && s->body ? s->body : "");
}

/* value of a request header (case-insensitive), or "" if absent. */
const char *vader_http_header(void *sv, const char *name) {
    HttpServer *s = sv;
    if (!s || !s->headers)
        return vader_strdup("");
    char needle[256];
    snprintf(needle, sizeof(needle), "\r\n%s:", name);
    char *p = strcasestr(s->headers, needle);
    if (!p)
        return vader_strdup("");
    p += strlen(needle);
    while (*p == ' ')
        p++;
    char *end = strstr(p, "\r\n");
    int len = end ? (int)(end - p) : (int)strlen(p);
    char *out = vader_alloc(len + 1);
    memcpy(out, p, len);
    out[len] = 0;
    return out;
}

/* sends the response and closes the current connection. */
void vader_http_respond(void *sv, long status, const char *ctype, const char *body) {
    HttpServer *s = sv;
    if (!s || s->cfd < 0)
        return;
    const char *reason = status == 200   ? "OK"
                         : status == 201 ? "Created"
                         : status == 204 ? "No Content"
                         : status == 400 ? "Bad Request"
                         : status == 401 ? "Unauthorized"
                         : status == 404 ? "Not Found"
                         : status == 500 ? "Internal Server Error"
                                         : "OK";
    int blen = (int)strlen(body);
    char head[1024];
    int hn = snprintf(head, sizeof(head),
                      "HTTP/1.1 %ld %s\r\nContent-Type: %s\r\nContent-Length: %d\r\n"
                      "Connection: close\r\n\r\n",
                      status, reason, ctype, blen);
    h_write_all(s->cfd, (const unsigned char *)head, hn);
    h_write_all(s->cfd, (const unsigned char *)body, blen);
    close(s->cfd);
    s->cfd = -1;
    /* no free() here: path/body/headers live in the request's arena,
       freed in bulk on the next accept. */
}

/* JSON is the default content type for APIs — respond without spelling it out. */
void vader_http_json(void *sv, long status, const char *body) {
    vader_http_respond(sv, status, "application/json", body);
}

/* plain-text convenience (health probes, simple endpoints). */
void vader_http_text(void *sv, long status, const char *body) {
    vader_http_respond(sv, status, "text/plain; charset=utf-8", body);
}

/* ===================== client ============================================ */
static const char *http_request(const char *method, const char *url,
                                const char *ctype, const char *body) {
    const char *p = url;
    if (strncmp(p, "http://", 7) == 0)
        p += 7;
    else if (strncmp(p, "https://", 8) == 0)
        return vader_strdup(""); /* v1: client without TLS */
    char host[256] = {0}, path[2048] = "/";
    int port = 80;
    const char *slash = strchr(p, '/');
    const char *hostend = slash ? slash : p + strlen(p);
    const char *colon = memchr(p, ':', hostend - p);
    if (colon) {
        memcpy(host, p, colon - p);
        host[colon - p] = 0;
        port = atoi(colon + 1);
    } else {
        memcpy(host, p, hostend - p);
        host[hostend - p] = 0;
    }
    if (slash)
        snprintf(path, sizeof(path), "%s", slash);

    struct addrinfo hints, *res;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    char ps[16];
    snprintf(ps, sizeof(ps), "%d", port);
    if (getaddrinfo(host, ps, &hints, &res) != 0)
        return vader_strdup("");
    int fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd < 0 || connect(fd, res->ai_addr, res->ai_addrlen) < 0) {
        freeaddrinfo(res);
        return vader_strdup("");
    }
    freeaddrinfo(res);

    char req[8192];
    int blen = body ? (int)strlen(body) : 0;
    int rn;
    if (body)
        rn = snprintf(req, sizeof(req),
                      "%s %s HTTP/1.1\r\nHost: %s\r\nContent-Type: %s\r\n"
                      "Content-Length: %d\r\nConnection: close\r\n\r\n",
                      method, path, host, ctype ? ctype : "text/plain", blen);
    else
        rn = snprintf(req, sizeof(req),
                      "%s %s HTTP/1.1\r\nHost: %s\r\nConnection: close\r\n\r\n",
                      method, path, host);
    h_write_all(fd, (const unsigned char *)req, rn);
    if (body)
        h_write_all(fd, (const unsigned char *)body, blen);

    /* reads the entire response */
    int cap = 65536, n = 0;
    char *resp = vader_alloc(cap);
    for (;;) {
        if (n >= cap - 1) {
            int oc = cap;
            cap *= 2;
            resp = vader_realloc(resp, oc, cap);
        }
        int r = read(fd, resp + n, cap - 1 - n);
        if (r <= 0)
            break;
        n += r;
    }
    resp[n] = 0;
    close(fd);
    char *bp = strstr(resp, "\r\n\r\n");
    return vader_strdup(bp ? bp + 4 : "");
}

const char *vader_http_get(const char *url) {
    return http_request("GET", url, 0, 0);
}
const char *vader_http_post(const char *url, const char *ctype, const char *body) {
    return http_request("POST", url, ctype, body);
}
