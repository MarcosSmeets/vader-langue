/* Vader HTTP router: newRouter() + r.get/post/...(path, handler) + serve(port).
 * Handlers are Vader functions `fn(Server)`, passed as plain function pointers. */
#include <stdlib.h>
#include <string.h>

extern void *vader_http_listen(long port);
extern int vader_http_accept(void *server);
extern const char *vader_http_method(void *server);
extern const char *vader_http_path(void *server);
extern void vader_http_respond(void *server, long status, const char *ctype, const char *body);

typedef void (*Handler)(void *server);

typedef struct {
    char *method;
    char *path;
    Handler fn;
} Route;

typedef struct {
    Route *routes;
    int count, cap;
} Router;

void *vader_router_new(void) {
    return calloc(1, sizeof(Router));
}

void vader_router_add(void *router, const char *method, const char *path, void *fn) {
    Router *r = router;
    if (r->count >= r->cap) {
        r->cap = r->cap ? r->cap * 2 : 8;
        r->routes = realloc(r->routes, r->cap * sizeof(Route));
    }
    r->routes[r->count].method = strdup(method);
    r->routes[r->count].path = strdup(path);
    r->routes[r->count].fn = (Handler)fn;
    r->count++;
}

/* listens on `port`, accepts forever, dispatches by method+path, else 404. */
void vader_router_serve(long port, void *router) {
    Router *r = router;
    void *srv = vader_http_listen(port);
    if (!srv)
        return;
    while (vader_http_accept(srv)) {
        const char *m = vader_http_method(srv);
        const char *p = vader_http_path(srv);
        int matched = 0;
        for (int i = 0; i < r->count; i++) {
            if (strcmp(r->routes[i].method, m) == 0 && strcmp(r->routes[i].path, p) == 0) {
                r->routes[i].fn(srv);
                matched = 1;
                break;
            }
        }
        if (!matched)
            vader_http_respond(srv, 404, "application/json", "{\"error\":\"not found\"}");
    }
}
