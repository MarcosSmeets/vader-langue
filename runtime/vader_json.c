/* JSON do runtime da Vader: árvore de valor + parse + encode + acessores/builders.
 * Self-contained. Sem-GC: tudo vaza, alinhado com o runtime. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { J_NULL, J_BOOL, J_INT, J_DBL, J_STR, J_ARR, J_OBJ };

typedef struct JVal {
    int type;
    long long i;
    double d;
    int b;
    char *s;
    struct JVal **items; /* arr: elementos; obj: valores (paralelo a keys) */
    char **keys;         /* só obj */
    int count, cap;
} JVal;

static JVal JNULL = {J_NULL, 0, 0, 0, 0, 0, 0, 0, 0};

static JVal *jnew(int type) {
    JVal *v = calloc(1, sizeof(JVal));
    v->type = type;
    return v;
}
static void jgrow(JVal *v) {
    if (v->count >= v->cap) {
        v->cap = v->cap ? v->cap * 2 : 4;
        v->items = realloc(v->items, v->cap * sizeof(JVal *));
        if (v->type == J_OBJ)
            v->keys = realloc(v->keys, v->cap * sizeof(char *));
    }
}

/* ===================== parser ============================================ */
static void jskip(const char **p) {
    while (**p == ' ' || **p == '\t' || **p == '\n' || **p == '\r')
        (*p)++;
}

static char *jparse_str(const char **p) {
    (*p)++; /* aspas de abertura */
    int cap = 16, n = 0;
    char *out = malloc(cap);
    while (**p && **p != '"') {
        if (n + 4 >= cap) {
            cap *= 2;
            out = realloc(out, cap);
        }
        if (**p == '\\') {
            (*p)++;
            char e = **p;
            (*p)++;
            switch (e) {
            case 'n': out[n++] = '\n'; break;
            case 't': out[n++] = '\t'; break;
            case 'r': out[n++] = '\r'; break;
            case 'b': out[n++] = '\b'; break;
            case 'f': out[n++] = '\f'; break;
            case '/': out[n++] = '/'; break;
            case '\\': out[n++] = '\\'; break;
            case '"': out[n++] = '"'; break;
            case 'u': {
                char hex[5] = {0};
                memcpy(hex, *p, 4);
                *p += 4;
                unsigned int cp = (unsigned int)strtol(hex, 0, 16);
                if (cp < 0x80) {
                    out[n++] = cp;
                } else if (cp < 0x800) {
                    out[n++] = 0xC0 | (cp >> 6);
                    out[n++] = 0x80 | (cp & 0x3F);
                } else {
                    out[n++] = 0xE0 | (cp >> 12);
                    out[n++] = 0x80 | ((cp >> 6) & 0x3F);
                    out[n++] = 0x80 | (cp & 0x3F);
                }
                break;
            }
            default: out[n++] = e; break;
            }
        } else {
            out[n++] = **p;
            (*p)++;
        }
    }
    if (**p == '"')
        (*p)++;
    out[n] = 0;
    return out;
}

static JVal *jparse_val(const char **p) {
    jskip(p);
    char c = **p;
    if (c == '{') {
        (*p)++;
        JVal *o = jnew(J_OBJ);
        jskip(p);
        if (**p == '}') { (*p)++; return o; }
        for (;;) {
            jskip(p);
            char *key = jparse_str(p);
            jskip(p);
            if (**p == ':') (*p)++;
            JVal *val = jparse_val(p);
            jgrow(o);
            o->keys[o->count] = key;
            o->items[o->count] = val;
            o->count++;
            jskip(p);
            if (**p == ',') { (*p)++; continue; }
            if (**p == '}') { (*p)++; }
            break;
        }
        return o;
    }
    if (c == '[') {
        (*p)++;
        JVal *a = jnew(J_ARR);
        jskip(p);
        if (**p == ']') { (*p)++; return a; }
        for (;;) {
            JVal *val = jparse_val(p);
            jgrow(a);
            a->items[a->count++] = val;
            jskip(p);
            if (**p == ',') { (*p)++; continue; }
            if (**p == ']') { (*p)++; }
            break;
        }
        return a;
    }
    if (c == '"') {
        JVal *v = jnew(J_STR);
        v->s = jparse_str(p);
        return v;
    }
    if (c == 't') { *p += 4; JVal *v = jnew(J_BOOL); v->b = 1; return v; }
    if (c == 'f') { *p += 5; JVal *v = jnew(J_BOOL); v->b = 0; return v; }
    if (c == 'n') { *p += 4; return &JNULL; }
    /* número */
    char *end;
    double d = strtod(*p, &end);
    JVal *v;
    /* inteiro se não tem . nem e/E no trecho consumido */
    int isint = 1;
    for (const char *q = *p; q < end; q++)
        if (*q == '.' || *q == 'e' || *q == 'E') { isint = 0; break; }
    if (isint) { v = jnew(J_INT); v->i = (long long)d; }
    else { v = jnew(J_DBL); v->d = d; }
    *p = end;
    return v;
}

void *vader_json_parse(const char *s) {
    const char *p = s;
    return jparse_val(&p);
}

/* ===================== acessores ========================================= */
void *vader_json_field(void *jv, const char *key) {
    JVal *v = jv;
    if (!v || v->type != J_OBJ) return &JNULL;
    for (int i = 0; i < v->count; i++)
        if (strcmp(v->keys[i], key) == 0) return v->items[i];
    return &JNULL;
}
void *vader_json_elem(void *jv, int idx) {
    JVal *v = jv;
    if (!v || v->type != J_ARR || idx < 0 || idx >= v->count) return &JNULL;
    return v->items[idx];
}
const char *vader_json_as_str(void *jv) {
    JVal *v = jv;
    if (!v) return strdup("");
    if (v->type == J_STR) return strdup(v->s);
    char buf[64];
    if (v->type == J_INT) { snprintf(buf, sizeof buf, "%lld", v->i); return strdup(buf); }
    if (v->type == J_DBL) { snprintf(buf, sizeof buf, "%g", v->d); return strdup(buf); }
    if (v->type == J_BOOL) return strdup(v->b ? "true" : "false");
    return strdup("");
}
long long vader_json_as_int(void *jv) {
    JVal *v = jv;
    if (!v) return 0;
    if (v->type == J_INT) return v->i;
    if (v->type == J_DBL) return (long long)v->d;
    if (v->type == J_BOOL) return v->b;
    if (v->type == J_STR) return atoll(v->s);
    return 0;
}
double vader_json_as_float(void *jv) {
    JVal *v = jv;
    if (!v) return 0;
    if (v->type == J_DBL) return v->d;
    if (v->type == J_INT) return (double)v->i;
    if (v->type == J_STR) return atof(v->s);
    return 0;
}
int vader_json_as_bool(void *jv) {
    JVal *v = jv;
    if (!v) return 0;
    if (v->type == J_BOOL) return v->b;
    if (v->type == J_INT) return v->i != 0;
    return 0;
}
long long vader_json_count(void *jv) {
    JVal *v = jv;
    return (v && (v->type == J_ARR || v->type == J_OBJ)) ? v->count : 0;
}

/* ===================== builders ========================================== */
void *vader_json_object(void) { return jnew(J_OBJ); }
void *vader_json_array(void) { return jnew(J_ARR); }

static void obj_set(JVal *o, const char *key, JVal *val) {
    if (!o || o->type != J_OBJ) return;
    for (int i = 0; i < o->count; i++)
        if (strcmp(o->keys[i], key) == 0) { o->items[i] = val; return; }
    jgrow(o);
    o->keys[o->count] = strdup(key);
    o->items[o->count] = val;
    o->count++;
}
void *vader_json_set(void *o, const char *key, void *child) { obj_set(o, key, child); return o; }
void *vader_json_set_str(void *o, const char *key, const char *val) {
    JVal *v = jnew(J_STR); v->s = strdup(val); obj_set(o, key, v); return o;
}
void *vader_json_set_int(void *o, const char *key, long long val) {
    JVal *v = jnew(J_INT); v->i = val; obj_set(o, key, v); return o;
}
void *vader_json_set_float(void *o, const char *key, double val) {
    JVal *v = jnew(J_DBL); v->d = val; obj_set(o, key, v); return o;
}
void *vader_json_set_bool(void *o, const char *key, int val) {
    JVal *v = jnew(J_BOOL); v->b = val ? 1 : 0; obj_set(o, key, v); return o;
}
static void arr_add(JVal *a, JVal *val) {
    if (!a || a->type != J_ARR) return;
    jgrow(a);
    a->items[a->count++] = val;
}
void *vader_json_add(void *a, void *child) { arr_add(a, child); return a; }
void *vader_json_add_str(void *a, const char *val) {
    JVal *v = jnew(J_STR); v->s = strdup(val); arr_add(a, v); return a;
}
void *vader_json_add_int(void *a, long long val) {
    JVal *v = jnew(J_INT); v->i = val; arr_add(a, v); return a;
}

/* ===================== encode ============================================ */
typedef struct { char *buf; int len, cap; } SB;
static void sb_putc(SB *s, char c) {
    if (s->len + 1 >= s->cap) { s->cap = s->cap ? s->cap * 2 : 64; s->buf = realloc(s->buf, s->cap); }
    s->buf[s->len++] = c;
}
static void sb_puts(SB *s, const char *str) {
    while (*str) sb_putc(s, *str++);
}
static void sb_json_str(SB *s, const char *str) {
    sb_putc(s, '"');
    for (const char *p = str; *p; p++) {
        switch (*p) {
        case '"': sb_puts(s, "\\\""); break;
        case '\\': sb_puts(s, "\\\\"); break;
        case '\n': sb_puts(s, "\\n"); break;
        case '\r': sb_puts(s, "\\r"); break;
        case '\t': sb_puts(s, "\\t"); break;
        default: sb_putc(s, *p);
        }
    }
    sb_putc(s, '"');
}
static void jencode(SB *s, JVal *v) {
    if (!v) { sb_puts(s, "null"); return; }
    char buf[64];
    switch (v->type) {
    case J_NULL: sb_puts(s, "null"); break;
    case J_BOOL: sb_puts(s, v->b ? "true" : "false"); break;
    case J_INT: snprintf(buf, sizeof buf, "%lld", v->i); sb_puts(s, buf); break;
    case J_DBL: snprintf(buf, sizeof buf, "%g", v->d); sb_puts(s, buf); break;
    case J_STR: sb_json_str(s, v->s); break;
    case J_ARR:
        sb_putc(s, '[');
        for (int i = 0; i < v->count; i++) { if (i) sb_putc(s, ','); jencode(s, v->items[i]); }
        sb_putc(s, ']');
        break;
    case J_OBJ:
        sb_putc(s, '{');
        for (int i = 0; i < v->count; i++) {
            if (i) sb_putc(s, ',');
            sb_json_str(s, v->keys[i]);
            sb_putc(s, ':');
            jencode(s, v->items[i]);
        }
        sb_putc(s, '}');
        break;
    }
}
const char *vader_json_encode(void *jv) {
    SB s = {0, 0, 0};
    jencode(&s, jv);
    sb_putc(&s, 0);
    return s.buf ? s.buf : strdup("null");
}
