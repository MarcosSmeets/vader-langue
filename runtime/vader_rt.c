// vader_rt.c — Vader concurrency runtime (channels + goroutines via pthreads).
// Linked by `clang` together with the generated LLVM IR. Memory leaks (no GC, for now).

#include <pthread.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ---- test mode: when running `vader test`, a panic unwinds to the test harness
// (longjmp) instead of aborting the process, so one failing assert fails just that
// test and the rest keep running. Outside tests, g_in_test is 0 and a panic exits. ----
int g_in_test = 0;
jmp_buf g_test_jmp;

// ---- runtime safety: panics print a message + location and abort (exit 1) ----
void vader_panic(const char *msg) {
    fprintf(stderr, "panic: %s\n", msg);
    if (g_in_test)
        longjmp(g_test_jmp, 1);
    exit(1);
}
// slice/array bounds check: aborts if `idx` is outside [0, len).
void vader_bounds(long long idx, long long len, long long line) {
    if (idx < 0 || idx >= len) {
        fprintf(stderr, "panic: index out of bounds at line %lld: index %lld, length %lld\n",
                line, idx, len);
        if (g_in_test)
            longjmp(g_test_jmp, 1);
        exit(1);
    }
}

typedef struct {
    char *buf;        // circular buffer: cap slots of elemsize bytes
    long elemsize;
    long cap;
    long count;
    long head;
    long tail;
    int closed;
    pthread_mutex_t mu;
    pthread_cond_t not_full;
    pthread_cond_t not_empty;
} VaderChan;

void *vader_chan_make(long elemsize, long cap) {
    if (cap < 1) cap = 1;
    VaderChan *c = (VaderChan *)malloc(sizeof(VaderChan));
    c->buf = (char *)malloc(elemsize * cap);
    c->elemsize = elemsize;
    c->cap = cap;
    c->count = 0;
    c->head = 0;
    c->tail = 0;
    c->closed = 0;
    pthread_mutex_init(&c->mu, NULL);
    pthread_cond_init(&c->not_full, NULL);
    pthread_cond_init(&c->not_empty, NULL);
    return (void *)c;
}

void vader_chan_send(void *ch, void *elem) {
    VaderChan *c = (VaderChan *)ch;
    pthread_mutex_lock(&c->mu);
    while (c->count == c->cap && !c->closed)
        pthread_cond_wait(&c->not_full, &c->mu);
    if (c->closed) {
        pthread_mutex_unlock(&c->mu);
        return;
    }
    memcpy(c->buf + c->tail * c->elemsize, elem, c->elemsize);
    c->tail = (c->tail + 1) % c->cap;
    c->count++;
    pthread_cond_signal(&c->not_empty);
    pthread_mutex_unlock(&c->mu);
}

// 1 if received; 0 if the channel is closed and empty.
int vader_chan_recv(void *ch, void *out) {
    VaderChan *c = (VaderChan *)ch;
    pthread_mutex_lock(&c->mu);
    while (c->count == 0 && !c->closed)
        pthread_cond_wait(&c->not_empty, &c->mu);
    if (c->count == 0 && c->closed) {
        pthread_mutex_unlock(&c->mu);
        return 0;
    }
    memcpy(out, c->buf + c->head * c->elemsize, c->elemsize);
    c->head = (c->head + 1) % c->cap;
    c->count--;
    pthread_cond_signal(&c->not_full);
    pthread_mutex_unlock(&c->mu);
    return 1;
}

void vader_chan_close(void *ch) {
    VaderChan *c = (VaderChan *)ch;
    pthread_mutex_lock(&c->mu);
    c->closed = 1;
    pthread_cond_broadcast(&c->not_empty);
    pthread_cond_broadcast(&c->not_full);
    pthread_mutex_unlock(&c->mu);
}

// spawns a goroutine (detached thread) that runs fn(arg).
void vader_go(void *(*fn)(void *), void *arg) {
    pthread_t t;
    pthread_create(&t, NULL, fn, arg);
    pthread_detach(t);
}

// ---- maps (hash table with chaining; int OR string key) ----

#define VMAP_BUCKETS 64

typedef struct VEntry {
    long ikey;
    char *skey; // NULL for int key
    void *val;
    struct VEntry *next;
} VEntry;

typedef struct {
    long valsize;
    int keyisstr;
    long count;
    VEntry *buckets[VMAP_BUCKETS];
} VaderMap;

static unsigned long vmap_hash_int(long k) {
    return (unsigned long)k * 2654435761UL;
}
static unsigned long vmap_hash_str(const char *s) {
    unsigned long h = 5381;
    while (*s)
        h = h * 33 + (unsigned char)*s++;
    return h;
}

void *vader_map_make(long valsize, int keyisstr) {
    VaderMap *m = (VaderMap *)calloc(1, sizeof(VaderMap));
    m->valsize = valsize;
    m->keyisstr = keyisstr;
    return m;
}

void vader_map_set_int(void *mp, long key, void *val) {
    VaderMap *m = (VaderMap *)mp;
    unsigned long b = vmap_hash_int(key) % VMAP_BUCKETS;
    for (VEntry *e = m->buckets[b]; e; e = e->next)
        if (!e->skey && e->ikey == key) {
            memcpy(e->val, val, m->valsize);
            return;
        }
    VEntry *e = (VEntry *)malloc(sizeof(VEntry));
    e->ikey = key;
    e->skey = NULL;
    e->val = malloc(m->valsize);
    memcpy(e->val, val, m->valsize);
    e->next = m->buckets[b];
    m->buckets[b] = e;
    m->count++;
}

int vader_map_get_int(void *mp, long key, void *out) {
    VaderMap *m = (VaderMap *)mp;
    unsigned long b = vmap_hash_int(key) % VMAP_BUCKETS;
    for (VEntry *e = m->buckets[b]; e; e = e->next)
        if (!e->skey && e->ikey == key) {
            memcpy(out, e->val, m->valsize);
            return 1;
        }
    memset(out, 0, m->valsize);
    return 0;
}

void vader_map_set_str(void *mp, char *key, void *val) {
    VaderMap *m = (VaderMap *)mp;
    unsigned long b = vmap_hash_str(key) % VMAP_BUCKETS;
    for (VEntry *e = m->buckets[b]; e; e = e->next)
        if (e->skey && strcmp(e->skey, key) == 0) {
            memcpy(e->val, val, m->valsize);
            return;
        }
    VEntry *e = (VEntry *)malloc(sizeof(VEntry));
    e->ikey = 0;
    e->skey = strdup(key);
    e->val = malloc(m->valsize);
    memcpy(e->val, val, m->valsize);
    e->next = m->buckets[b];
    m->buckets[b] = e;
    m->count++;
}

int vader_map_get_str(void *mp, char *key, void *out) {
    VaderMap *m = (VaderMap *)mp;
    unsigned long b = vmap_hash_str(key) % VMAP_BUCKETS;
    for (VEntry *e = m->buckets[b]; e; e = e->next)
        if (e->skey && strcmp(e->skey, key) == 0) {
            memcpy(out, e->val, m->valsize);
            return 1;
        }
    memset(out, 0, m->valsize);
    return 0;
}

long vader_map_len(void *mp) { return ((VaderMap *)mp)->count; }
