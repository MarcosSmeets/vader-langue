/* Vader runtime std/fmt: value -> string conversions (no varargs in v1). */
#include <stdio.h>

extern void *vader_alloc(long n);

char *vader_fmt_from_int(long long n) {
    char *o = vader_alloc(24);
    snprintf(o, 24, "%lld", n);
    return o;
}
char *vader_fmt_from_float(double f) {
    char *o = vader_alloc(32);
    snprintf(o, 32, "%g", f);
    return o;
}
char *vader_fmt_from_bool(int b) {
    char *o = vader_alloc(6);
    snprintf(o, 6, "%s", b ? "true" : "false");
    return o;
}
/* left-pad `s` with `ch` to at least `width` characters. */
char *vader_fmt_pad_left(const char *s, long long width, const char *ch) {
    char fill = (ch && ch[0]) ? ch[0] : ' ';
    long long n = 0;
    const char *p = s;
    while (*p++) n++;
    long long pad = width > n ? width - n : 0;
    char *o = vader_alloc(pad + n + 1);
    long long i = 0;
    for (; i < pad; i++) o[i] = fill;
    for (long long k = 0; k < n; k++) o[i++] = s[k];
    o[i] = 0;
    return o;
}
