/* Diagnóstico: o runtime (arena + json) vaza por iteração? */
#include <stdio.h>
#include <malloc.h>

extern void *vader_scope(void);
extern void vader_reset(void *);
extern void *vader_json_object(void);
extern void *vader_json_set_str(void *, const char *, const char *);
extern const char *vader_json_encode(void *);
extern char *vader_strdup(const char *);

int main(void) {
    void *a = vader_scope();
    /* warmup */
    for (int i = 0; i < 100; i++) {
        vader_reset(a);
        void *o = vader_json_object();
        vader_json_set_str(o, "path", "/x");
        vader_json_set_str(o, "msg", "ola");
        vader_json_encode(o);
    }
    struct mallinfo2 m0 = mallinfo2();
    for (int i = 0; i < 100000; i++) {
        vader_reset(a);
        void *o = vader_json_object();
        vader_json_set_str(o, "path", "/x");
        vader_json_set_str(o, "msg", "ola");
        vader_json_encode(o);
    }
    struct mallinfo2 m1 = mallinfo2();
    long d = (long)m1.uordblks - (long)m0.uordblks;
    printf("uordblks delta: %ld bytes / 100000 iters = %.3f bytes/iter\n", d, d / 100000.0);
    return 0;
}
