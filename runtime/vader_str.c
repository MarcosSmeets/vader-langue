/* Vader runtime std/strings: string utilities. Strings are NUL-terminated i8*;
 * results are allocated in the arena (vader_alloc), in line with the runtime. */
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

extern void *vader_alloc(long n);
extern char *vader_strdup(const char *s);

/* a Vader slice value: { ptr, len } — matches []string in the IR */
typedef struct { char **ptr; long long len; } VSlice;

long long vader_str_length(const char *s) { return (long long)strlen(s); }

char *vader_str_upper(const char *s) {
    int n = (int)strlen(s);
    char *o = vader_alloc(n + 1);
    for (int i = 0; i < n; i++) o[i] = (char)toupper((unsigned char)s[i]);
    o[n] = 0;
    return o;
}
char *vader_str_lower(const char *s) {
    int n = (int)strlen(s);
    char *o = vader_alloc(n + 1);
    for (int i = 0; i < n; i++) o[i] = (char)tolower((unsigned char)s[i]);
    o[n] = 0;
    return o;
}
char *vader_str_trim(const char *s) {
    while (*s && isspace((unsigned char)*s)) s++;
    int n = (int)strlen(s);
    while (n > 0 && isspace((unsigned char)s[n - 1])) n--;
    char *o = vader_alloc(n + 1);
    memcpy(o, s, n);
    o[n] = 0;
    return o;
}
int vader_str_contains(const char *s, const char *sub) { return strstr(s, sub) != 0; }
long long vader_str_index_of(const char *s, const char *sub) {
    const char *p = strstr(s, sub);
    return p ? (long long)(p - s) : -1;
}
int vader_str_starts_with(const char *s, const char *pre) {
    return strncmp(s, pre, strlen(pre)) == 0;
}
int vader_str_ends_with(const char *s, const char *suf) {
    int ls = (int)strlen(s), lf = (int)strlen(suf);
    return ls >= lf && strcmp(s + ls - lf, suf) == 0;
}
char *vader_str_substring(const char *s, long long start, long long end) {
    int n = (int)strlen(s);
    if (start < 0) start = 0;
    if (end > n) end = n;
    if (end < start) end = start;
    int len = (int)(end - start);
    char *o = vader_alloc(len + 1);
    memcpy(o, s + start, len);
    o[len] = 0;
    return o;
}
char *vader_str_repeat(const char *s, long long times) {
    int n = (int)strlen(s);
    if (times < 0) times = 0;
    char *o = vader_alloc(n * times + 1);
    int p = 0;
    for (long long t = 0; t < times; t++) { memcpy(o + p, s, n); p += n; }
    o[p] = 0;
    return o;
}
char *vader_str_replace(const char *s, const char *old, const char *neww) {
    int lo = (int)strlen(old);
    if (lo == 0) return vader_strdup(s);
    int ln = (int)strlen(neww);
    int cnt = 0;
    const char *p = s;
    while ((p = strstr(p, old))) { cnt++; p += lo; }
    int outlen = (int)strlen(s) + cnt * (ln - lo);
    char *o = vader_alloc(outlen + 1);
    int oi = 0;
    p = s;
    while (*p) {
        if (strncmp(p, old, lo) == 0) { memcpy(o + oi, neww, ln); oi += ln; p += lo; }
        else o[oi++] = *p++;
    }
    o[oi] = 0;
    return o;
}
long long vader_str_to_int(const char *s) { return atoll(s); }
double vader_str_to_float(const char *s) { return atof(s); }

VSlice vader_str_split(const char *s, const char *sep) {
    VSlice r;
    int lsep = (int)strlen(sep);
    if (lsep == 0) {
        r.ptr = vader_alloc(sizeof(char *));
        r.ptr[0] = vader_strdup(s);
        r.len = 1;
        return r;
    }
    int cnt = 1;
    const char *p = s;
    while ((p = strstr(p, sep))) { cnt++; p += lsep; }
    char **arr = vader_alloc(cnt * sizeof(char *));
    int idx = 0;
    const char *start = s;
    for (;;) {
        const char *hit = strstr(start, sep);
        int len = hit ? (int)(hit - start) : (int)strlen(start);
        char *piece = vader_alloc(len + 1);
        memcpy(piece, start, len);
        piece[len] = 0;
        arr[idx++] = piece;
        if (!hit) break;
        start = hit + lsep;
    }
    r.ptr = arr;
    r.len = idx;
    return r;
}

char *vader_str_join(VSlice parts, const char *sep) {
    int lsep = (int)strlen(sep);
    long long total = 0;
    for (long long i = 0; i < parts.len; i++) total += (long long)strlen(parts.ptr[i]) + (i ? lsep : 0);
    char *o = vader_alloc(total + 1);
    int oi = 0;
    for (long long i = 0; i < parts.len; i++) {
        if (i) { memcpy(o + oi, sep, lsep); oi += lsep; }
        int l = (int)strlen(parts.ptr[i]);
        memcpy(o + oi, parts.ptr[i], l);
        oi += l;
    }
    o[oi] = 0;
    return o;
}
