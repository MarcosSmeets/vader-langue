/* Vader runtime std/mongo: a minimal MongoDB client (BSON + OP_MSG, opcode 2013).
 * Document API: connect / insert / find / close. No auth (local/dev Mongo).
 * Reuses the vader_json value tree for documents. Little-endian host assumed (x86_64). */
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>

/* json tree accessors + builders (vader_json.c) */
extern int vader_json_type(void *);
extern int vader_json_keycount(void *);
extern const char *vader_json_key_at(void *, int);
extern void *vader_json_value_at(void *, int);
extern const char *vader_json_as_str(void *);
extern long long vader_json_as_int(void *);
extern double vader_json_as_float(void *);
extern int vader_json_as_bool(void *);
extern void *vader_json_object(void);
extern void *vader_json_array(void);
extern void *vader_json_set(void *, const char *, void *);
extern void *vader_json_set_str(void *, const char *, const char *);
extern void *vader_json_set_int(void *, const char *, long long);
extern void *vader_json_set_float(void *, const char *, double);
extern void *vader_json_set_bool(void *, const char *, int);
extern void *vader_json_add(void *, void *);
extern void *vader_json_add_str(void *, const char *);
extern void *vader_json_add_int(void *, long long);
extern void *vader_json_add_float(void *, double);
extern void *vader_json_add_bool(void *, int);
extern void *vader_json_field(void *, const char *);
extern char *vader_strdup(const char *);

enum { JT_NULL, JT_BOOL, JT_INT, JT_DBL, JT_STR, JT_ARR, JT_OBJ };

/* ---- growable byte buffer (malloc; freed after the request) ---- */
typedef struct { unsigned char *buf; int len, cap; } BB;
static void bb_need(BB *b, int n) {
    if (b->len + n > b->cap) {
        if (!b->cap) b->cap = 128;
        while (b->cap < b->len + n) b->cap *= 2;
        b->buf = realloc(b->buf, b->cap);
    }
}
static void bb_u8(BB *b, unsigned char c) { bb_need(b, 1); b->buf[b->len++] = c; }
static void bb_raw(BB *b, const void *p, int n) { bb_need(b, n); memcpy(b->buf + b->len, p, n); b->len += n; }
static void bb_i32(BB *b, int v) { bb_raw(b, &v, 4); }
static void bb_i64(BB *b, long long v) { bb_raw(b, &v, 8); }
static void bb_cstr(BB *b, const char *s) { bb_raw(b, s, (int)strlen(s) + 1); }

/* ---- encode a json tree to BSON ---- */
static void bson_doc(BB *b, void *j);

static void bson_elem(BB *b, const char *key, void *j) {
    switch (vader_json_type(j)) {
    case JT_DBL: { bb_u8(b, 0x01); bb_cstr(b, key); double d = vader_json_as_float(j); bb_raw(b, &d, 8); break; }
    case JT_STR: { bb_u8(b, 0x02); bb_cstr(b, key); const char *s = vader_json_as_str(j);
                   int l = (int)strlen(s) + 1; bb_i32(b, l); bb_raw(b, s, l); break; }
    case JT_OBJ: { bb_u8(b, 0x03); bb_cstr(b, key); bson_doc(b, j); break; }
    case JT_ARR: { bb_u8(b, 0x04); bb_cstr(b, key); bson_doc(b, j); break; }
    case JT_BOOL: { bb_u8(b, 0x08); bb_cstr(b, key); bb_u8(b, vader_json_as_bool(j) ? 1 : 0); break; }
    case JT_INT: { bb_u8(b, 0x12); bb_cstr(b, key); bb_i64(b, vader_json_as_int(j)); break; }
    default: { bb_u8(b, 0x0A); bb_cstr(b, key); break; }  /* null */
    }
}

static void bson_doc(BB *b, void *j) {
    int start = b->len;
    bb_i32(b, 0);  /* length placeholder */
    int n = vader_json_keycount(j);
    int isarr = vader_json_type(j) == JT_ARR;
    char idx[16];
    for (int i = 0; i < n; i++) {
        const char *key;
        if (isarr) { snprintf(idx, sizeof idx, "%d", i); key = idx; }
        else key = vader_json_key_at(j, i);
        bson_elem(b, key, vader_json_value_at(j, i));
    }
    bb_u8(b, 0);  /* terminator */
    int len = b->len - start;
    memcpy(b->buf + start, &len, 4);
}

/* ---- decode BSON to a json tree ---- */
static int rd_i32(const unsigned char *p) { int v; memcpy(&v, p, 4); return v; }
static long long rd_i64(const unsigned char *p) { long long v; memcpy(&v, p, 8); return v; }

static void *bson_decode(const unsigned char *p, int as_arr);

static void decode_elem(void *parent, int as_arr, const char *key, unsigned char type, const unsigned char **pp) {
    const unsigned char *p = *pp;
    switch (type) {
    case 0x01: { double d; memcpy(&d, p, 8); *pp += 8;
        if (as_arr) vader_json_add_float(parent, d); else vader_json_set_float(parent, key, d); break; }
    case 0x02: { int l = rd_i32(p); const char *s = (const char *)(p + 4); *pp += 4 + l;
        if (as_arr) vader_json_add_str(parent, s); else vader_json_set_str(parent, key, s); break; }
    case 0x03: case 0x04: { void *child = bson_decode(p, type == 0x04); *pp += rd_i32(p);
        if (as_arr) vader_json_add(parent, child); else vader_json_set(parent, key, child); break; }
    case 0x07: { char hex[25]; for (int k = 0; k < 12; k++) snprintf(hex + k * 2, 3, "%02x", p[k]); *pp += 12;
        if (as_arr) vader_json_add_str(parent, hex); else vader_json_set_str(parent, key, hex); break; }
    case 0x08: { int v = *p; *pp += 1;
        if (as_arr) vader_json_add_bool(parent, v); else vader_json_set_bool(parent, key, v); break; }
    case 0x09: { long long v = rd_i64(p); *pp += 8;  /* datetime (ms) -> int */
        if (as_arr) vader_json_add_int(parent, v); else vader_json_set_int(parent, key, v); break; }
    case 0x10: { int v = rd_i32(p); *pp += 4;
        if (as_arr) vader_json_add_int(parent, v); else vader_json_set_int(parent, key, v); break; }
    case 0x12: { long long v = rd_i64(p); *pp += 8;
        if (as_arr) vader_json_add_int(parent, v); else vader_json_set_int(parent, key, v); break; }
    default: {  /* null / unsupported */
        if (as_arr) vader_json_add_str(parent, ""); else vader_json_set_str(parent, key, ""); break; }
    }
}

static void *bson_decode(const unsigned char *p, int as_arr) {
    void *obj = as_arr ? vader_json_array() : vader_json_object();
    int len = rd_i32(p);
    const unsigned char *q = p + 4;
    const unsigned char *end = p + len;
    while (q < end - 1 && *q) {
        unsigned char type = *q++;
        const char *key = (const char *)q;
        q += strlen(key) + 1;
        decode_elem(obj, as_arr, key, type, &q);
    }
    return obj;
}

/* ---- socket + OP_MSG ---- */
static int read_n(int fd, unsigned char *p, int n) {
    int got = 0;
    while (got < n) {
        int r = (int)read(fd, p + got, n - got);
        if (r <= 0) return -1;
        got += r;
    }
    return 0;
}

typedef struct { int fd; char db[128]; } Mongo;

/* sends `cmd` as an OP_MSG command, returns the decoded reply document or 0. */
static void *op_msg(Mongo *m, void *cmd) {
    BB body = {0, 0, 0};
    bb_i32(&body, 0);  /* flagBits */
    bb_u8(&body, 0);   /* section kind 0 */
    bson_doc(&body, cmd);
    BB msg = {0, 0, 0};
    bb_i32(&msg, 16 + body.len);
    bb_i32(&msg, 1);     /* requestID */
    bb_i32(&msg, 0);     /* responseTo */
    bb_i32(&msg, 2013);  /* OP_MSG */
    bb_raw(&msg, body.buf, body.len);
    int ok = (write(m->fd, msg.buf, msg.len) == (ssize_t)msg.len);
    free(body.buf);
    free(msg.buf);
    if (!ok) return 0;
    unsigned char hdr[16];
    if (read_n(m->fd, hdr, 16) < 0) return 0;
    int blen = rd_i32(hdr) - 16;
    if (blen <= 5) return 0;
    unsigned char *rb = malloc(blen);
    if (read_n(m->fd, rb, blen) < 0) { free(rb); return 0; }
    void *reply = bson_decode(rb + 5, 0);  /* skip flagBits(4) + section kind(1) */
    free(rb);
    return reply;
}

void *vader_mongo_connect(const char *dsn) {
    char host[128] = "127.0.0.1", db[128] = "test";
    int port = 27017;
    const char *p = strstr(dsn, "://");
    p = p ? p + 3 : dsn;
    const char *at = strchr(p, '@');
    if (at) p = at + 1;  /* skip user:pass@ */
    const char *slash = strchr(p, '/');
    const char *hostend = slash ? slash : p + strlen(p);
    const char *colon = memchr(p, ':', hostend - p);
    if (colon) { int hl = (int)(colon - p); memcpy(host, p, hl); host[hl] = 0; port = atoi(colon + 1); }
    else { int hl = (int)(hostend - p); if (hl > 0 && hl < 127) { memcpy(host, p, hl); host[hl] = 0; } }
    if (slash) {
        const char *q = strchr(slash + 1, '?');
        int dl = q ? (int)(q - slash - 1) : (int)strlen(slash + 1);
        if (dl > 0 && dl < 127) { memcpy(db, slash + 1, dl); db[dl] = 0; }
    }
    char portstr[16];
    snprintf(portstr, sizeof portstr, "%d", port);
    struct addrinfo hints, *res = 0;
    memset(&hints, 0, sizeof hints);
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    if (getaddrinfo(host, portstr, &hints, &res) != 0 || !res) return 0;
    int fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd < 0 || connect(fd, res->ai_addr, res->ai_addrlen) < 0) {
        if (fd >= 0) close(fd);
        freeaddrinfo(res);
        return 0;
    }
    freeaddrinfo(res);
    Mongo *m = calloc(1, sizeof(Mongo));
    m->fd = fd;
    strncpy(m->db, db, 127);
    return m;
}

const char *vader_mongo_insert(void *mh, const char *coll, void *doc) {
    Mongo *m = mh;
    if (!m) return vader_strdup("mongo: not connected");
    void *cmd = vader_json_object();
    vader_json_set_str(cmd, "insert", coll);
    void *docs = vader_json_array();
    vader_json_add(docs, doc);
    vader_json_set(cmd, "documents", docs);
    vader_json_set_str(cmd, "$db", m->db);
    void *reply = op_msg(m, cmd);
    if (!reply) return vader_strdup("mongo: request failed");
    void *we = vader_json_field(reply, "writeErrors");
    if (vader_json_type(we) == JT_ARR && vader_json_keycount(we) > 0)
        return vader_strdup("mongo: write error");
    return 0;
}

void *vader_mongo_find(void *mh, const char *coll, void *query) {
    Mongo *m = mh;
    if (!m) return vader_json_array();
    void *cmd = vader_json_object();
    vader_json_set_str(cmd, "find", coll);
    vader_json_set(cmd, "filter", query);
    vader_json_set_str(cmd, "$db", m->db);
    void *reply = op_msg(m, cmd);
    if (!reply) return vader_json_array();
    void *batch = vader_json_field(vader_json_field(reply, "cursor"), "firstBatch");
    if (vader_json_type(batch) != JT_ARR) return vader_json_array();
    return batch;
}

void vader_mongo_close(void *mh) {
    Mongo *m = mh;
    if (m) {
        if (m->fd >= 0) close(m->fd);
        free(m);
    }
}
