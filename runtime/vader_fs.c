/* Vader runtime std/fs: file read/write and stdin. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern void *vader_alloc(long n);
extern char *vader_strdup(const char *s);

/* whole-file read; returns "" if the file can't be opened. */
char *vader_fs_read_file(const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) return vader_strdup("");
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    if (n < 0) n = 0;
    fseek(f, 0, SEEK_SET);
    char *o = vader_alloc(n + 1);
    size_t rd = fread(o, 1, (size_t)n, f);
    o[rd] = 0;
    fclose(f);
    return o;
}
int vader_fs_write_file(const char *path, const char *content) {
    FILE *f = fopen(path, "wb");
    if (!f) return 0;
    fputs(content, f);
    fclose(f);
    return 1;
}
int vader_fs_append_file(const char *path, const char *content) {
    FILE *f = fopen(path, "ab");
    if (!f) return 0;
    fputs(content, f);
    fclose(f);
    return 1;
}
int vader_fs_exists(const char *path) {
    FILE *f = fopen(path, "rb");
    if (f) { fclose(f); return 1; }
    return 0;
}
int vader_fs_remove(const char *path) { return remove(path) == 0; }

/* reads one line from stdin (without the trailing newline); "" at EOF. */
char *vader_fs_read_line(void) {
    size_t cap = 256, len = 0;
    char *buf = vader_alloc(cap);
    int ch;
    while ((ch = fgetc(stdin)) != EOF && ch != '\n') {
        if (len + 1 >= cap) {
            size_t nc = cap * 2;
            char *nb = vader_alloc(nc);
            memcpy(nb, buf, len);
            buf = nb;
            cap = nc;
        }
        buf[len++] = (char)ch;
    }
    buf[len] = 0;
    return buf;
}
