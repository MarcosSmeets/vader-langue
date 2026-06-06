/* Vader runtime arena/region allocator.
 *
 * Model: each "scope" (HTTP request, worker job) is an arena of blocks with
 * bump-allocation; the whole block is freed at once at the end. No GC,
 * deterministic — in line with the real-time/embedded vision.
 *
 * `vader_alloc` uses the current arena (thread-local). With NO active arena, it falls back to malloc
 * (leaks on purpose — embedded mode). `vader_scope`/`vader_release` stack. */
#include <stdlib.h>
#include <string.h>

typedef struct Block {
    struct Block *next;
    char *base;
    unsigned long used, cap;
} Block;

typedef struct Arena {
    Block *head;
    struct Arena *prev;
} Arena;

static __thread Arena *g_cur = 0;

static Block *block_new(unsigned long min) {
    unsigned long cap = min > 65536 ? min : 65536;
    Block *b = (Block *)malloc(sizeof(Block));
    b->base = (char *)malloc(cap);
    b->used = 0;
    b->cap = cap;
    b->next = 0;
    return b;
}

void *vader_alloc(long n) {
    unsigned long sz = n > 0 ? (unsigned long)n : 1;
    sz = (sz + 15) & ~15UL; /* align to 16 */
    Arena *a = g_cur;
    if (!a)
        return malloc(sz); /* no scope: malloc (leaks; embedded/real-time mode) */
    if (!a->head || a->head->used + sz > a->head->cap) {
        Block *b = block_new(sz);
        b->next = a->head;
        a->head = b;
    }
    void *p = a->head->base + a->head->used;
    a->head->used += sz;
    return p;
}

void *vader_realloc(void *old, long oldn, long newn) {
    void *p = vader_alloc(newn);
    if (old && oldn > 0) {
        unsigned long copy = (oldn < newn ? oldn : newn);
        memcpy(p, old, copy);
    }
    return p;
}

char *vader_strdup(const char *s) {
    if (!s)
        s = "";
    unsigned long n = strlen(s) + 1;
    char *p = (char *)vader_alloc((long)n);
    memcpy(p, s, n);
    return p;
}

/* opens a new memory scope; returns the arena handle. */
void *vader_scope(void) {
    Arena *a = (Arena *)malloc(sizeof(Arena));
    a->head = 0;
    a->prev = g_cur;
    g_cur = a;
    return a;
}

/* reuses the arena: resets the blocks (keeps the capacity) and makes it current.
   Avoids malloc/free churn when the same scope is reused each iteration. */
void vader_reset(void *arena) {
    Arena *a = (Arena *)arena;
    if (!a)
        return;
    for (Block *b = a->head; b; b = b->next)
        b->used = 0;
    g_cur = a;
}

/* frees everything in the scope and restores the previous one. */
void vader_release(void *arena) {
    Arena *a = (Arena *)arena;
    if (!a)
        return;
    Block *b = a->head;
    while (b) {
        Block *n = b->next;
        free(b->base);
        free(b);
        b = n;
    }
    if (g_cur == a)
        g_cur = a->prev;
    free(a);
}
