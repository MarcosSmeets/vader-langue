/* Alocador de arena/região do runtime da Vader.
 *
 * Modelo: cada "escopo" (request HTTP, job de worker) é uma arena de blocos com
 * bump-allocation; libera-se o bloco inteiro de uma vez no fim. Sem-GC,
 * determinístico — alinhado com a visão tempo-real/embarcado.
 *
 * `vader_alloc` usa a arena atual (thread-local). SEM arena ativa, cai em malloc
 * (vaza de propósito — modo embedded). `vader_scope`/`vader_release` empilham. */
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
    sz = (sz + 15) & ~15UL; /* alinha em 16 */
    Arena *a = g_cur;
    if (!a)
        return malloc(sz); /* sem escopo: malloc (vaza; modo embedded/real-time) */
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

/* abre um novo escopo de memória; retorna o handle da arena. */
void *vader_scope(void) {
    Arena *a = (Arena *)malloc(sizeof(Arena));
    a->head = 0;
    a->prev = g_cur;
    g_cur = a;
    return a;
}

/* reaproveita a arena: zera os blocos (mantém a capacidade) e a torna a atual.
   Evita churn de malloc/free quando o mesmo escopo é reusado a cada iteração. */
void vader_reset(void *arena) {
    Arena *a = (Arena *)arena;
    if (!a)
        return;
    for (Block *b = a->head; b; b = b->next)
        b->used = 0;
    g_cur = a;
}

/* libera tudo do escopo e restaura o anterior. */
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
