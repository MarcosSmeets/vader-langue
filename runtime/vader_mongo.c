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

/* SCRAM-SHA-256 crypto (vader_scram.c) */
extern void vader_scram_sha256(const unsigned char *, int, unsigned char[32]);
extern void vader_scram_hmac(const unsigned char *, int, const unsigned char *, int, unsigned char[32]);
extern void vader_scram_pbkdf2(const char *, int, const unsigned char *, int, int, unsigned char[32]);
extern char *vader_scram_b64encode(const unsigned char *, int);
extern int vader_scram_b64decode(const char *, int, unsigned char *);
extern void vader_scram_nonce(char *, int);

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

/* sends a raw BSON command doc as OP_MSG; returns the reply BSON document (malloc) + len. */
static unsigned char *op_msg_raw(Mongo *m, const unsigned char *cmd, int cmdlen, int *outlen) {
    BB body = {0, 0, 0};
    bb_i32(&body, 0);  /* flagBits */
    bb_u8(&body, 0);   /* section kind 0 */
    bb_raw(&body, cmd, cmdlen);
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
    int doclen = blen - 5;  /* skip flagBits(4) + section kind(1) */
    unsigned char *doc = malloc(doclen);
    memcpy(doc, rb + 5, doclen);
    free(rb);
    if (outlen) *outlen = doclen;
    return doc;
}

/* sends a json command, returns the decoded reply document or 0. */
static void *op_msg(Mongo *m, void *cmd) {
    BB b = {0, 0, 0};
    bson_doc(&b, cmd);
    int rlen;
    unsigned char *doc = op_msg_raw(m, b.buf, b.len, &rlen);
    free(b.buf);
    if (!doc) return 0;
    void *reply = bson_decode(doc, 0);
    free(doc);
    return reply;
}

/* ---- raw BSON field scan (for binary auth payloads the json tree can't hold) ---- */
static int bson_value_size(unsigned char type, const unsigned char *p) {
    switch (type) {
    case 0x01: return 8;             /* double */
    case 0x02: return 4 + rd_i32(p); /* string */
    case 0x03: case 0x04: return rd_i32(p);  /* doc/array */
    case 0x05: return 5 + rd_i32(p); /* binary */
    case 0x07: return 12;            /* objectid */
    case 0x08: return 1;             /* bool */
    case 0x09: return 8;             /* datetime */
    case 0x10: return 4;             /* int32 */
    case 0x11: return 8;             /* timestamp */
    case 0x12: return 8;             /* int64 */
    case 0x0A: return 0;             /* null */
    default: return -1;
    }
}

static const unsigned char *bson_scan(const unsigned char *doc, const char *key, unsigned char *type_out) {
    int len = rd_i32(doc);
    const unsigned char *q = doc + 4;
    const unsigned char *end = doc + len;
    while (q < end - 1 && *q) {
        unsigned char type = *q++;
        const char *k = (const char *)q;
        q += strlen(k) + 1;
        int sz = bson_value_size(type, q);
        if (sz < 0) return 0;
        if (strcmp(k, key) == 0) { if (type_out) *type_out = type; return q; }
        q += sz;
    }
    return 0;
}

/* builds a saslStart/saslContinue BSON command into `out` (a binary `payload`). */
static int build_sasl(BB *out, const char *cmdname, int convid, const char *payload, int plen, const char *db) {
    int start = out->len;
    bb_i32(out, 0);
    bb_u8(out, 0x10); bb_cstr(out, cmdname); bb_i32(out, 1);
    if (convid >= 0) { bb_u8(out, 0x10); bb_cstr(out, "conversationId"); bb_i32(out, convid); }
    else { bb_u8(out, 0x02); bb_cstr(out, "mechanism");
           const char *mech = "SCRAM-SHA-256"; int l = (int)strlen(mech) + 1; bb_i32(out, l); bb_raw(out, mech, l); }
    bb_u8(out, 0x05); bb_cstr(out, "payload"); bb_i32(out, plen); bb_u8(out, 0x00); bb_raw(out, payload, plen);
    bb_u8(out, 0x02); bb_cstr(out, "$db"); int dl = (int)strlen(db) + 1; bb_i32(out, dl); bb_raw(out, db, dl);
    bb_u8(out, 0);
    int len = out->len - start;
    memcpy(out->buf + start, &len, 4);
    return start;
}

/* SCRAM-SHA-256 authentication over saslStart/saslContinue. Returns 0 on success. */
static int mongo_auth(Mongo *m, const char *user, const char *pass) {
    const char *authdb = "admin";  /* SCRAM credentials live in admin by default */
    char cnonce[33];
    vader_scram_nonce(cnonce, 24);
    char first_bare[220];
    snprintf(first_bare, sizeof first_bare, "n=%s,r=%s", user, cnonce);
    char gs2[280];
    snprintf(gs2, sizeof gs2, "n,,%s", first_bare);

    BB c1 = {0, 0, 0};
    build_sasl(&c1, "saslStart", -1, gs2, (int)strlen(gs2), authdb);
    int rlen;
    unsigned char *r1 = op_msg_raw(m, c1.buf, c1.len, &rlen);
    free(c1.buf);
    if (!r1) return -1;

    unsigned char t;
    const unsigned char *cidp = bson_scan(r1, "conversationId", &t);
    int convid = cidp ? rd_i32(cidp) : 1;
    const unsigned char *plp = bson_scan(r1, "payload", &t);
    if (!plp) { free(r1); return -1; }
    int sflen = rd_i32(plp);
    char server_first[420];
    if (sflen > 419) sflen = 419;
    memcpy(server_first, plp + 5, sflen);
    server_first[sflen] = 0;
    free(r1);

    char rnonce[220] = {0}, salt_b64[220] = {0};
    int iter = 4096;
    char *rr = strstr(server_first, "r="), *ss = strstr(server_first, "s="), *ii = strstr(server_first, "i=");
    if (!rr || !ss || !ii) return -1;
    sscanf(rr + 2, "%219[^,]", rnonce);
    sscanf(ss + 2, "%219[^,]", salt_b64);
    iter = atoi(ii + 2);
    unsigned char salt[200];
    int saltlen = vader_scram_b64decode(salt_b64, (int)strlen(salt_b64), salt);

    unsigned char salted[32], ckey[32], skey[32], csig[32], proof[32];
    vader_scram_pbkdf2(pass, (int)strlen(pass), salt, saltlen, iter, salted);
    vader_scram_hmac(salted, 32, (const unsigned char *)"Client Key", 10, ckey);
    vader_scram_sha256(ckey, 32, skey);
    char final_noproof[280];
    snprintf(final_noproof, sizeof final_noproof, "c=biws,r=%s", rnonce);
    char authmsg[900];
    snprintf(authmsg, sizeof authmsg, "%s,%s,%s", first_bare, server_first, final_noproof);
    vader_scram_hmac(skey, 32, (const unsigned char *)authmsg, (int)strlen(authmsg), csig);
    for (int i = 0; i < 32; i++) proof[i] = ckey[i] ^ csig[i];
    char *pb = vader_scram_b64encode(proof, 32);
    char client_final[420];
    snprintf(client_final, sizeof client_final, "%s,p=%s", final_noproof, pb);
    free(pb);

    BB c2 = {0, 0, 0};
    build_sasl(&c2, "saslContinue", convid, client_final, (int)strlen(client_final), authdb);
    unsigned char *r2 = op_msg_raw(m, c2.buf, c2.len, &rlen);
    free(c2.buf);
    if (!r2) return -1;
    const unsigned char *okp = bson_scan(r2, "ok", &t);
    double ok = 0;
    if (okp) { if (t == 0x01) memcpy(&ok, okp, 8); else if (t == 0x10) ok = rd_i32(okp); }
    const unsigned char *donep = bson_scan(r2, "done", &t);
    int done = donep ? *donep : 0;
    free(r2);
    if (ok < 0.5) return -1;
    if (done) return 0;
    /* server sent the server-final but the conversation isn't done: finish with an
     * empty saslContinue (the client confirms it verified the server signature). */
    BB c3 = {0, 0, 0};
    build_sasl(&c3, "saslContinue", convid, "", 0, authdb);
    unsigned char *r3 = op_msg_raw(m, c3.buf, c3.len, &rlen);
    free(c3.buf);
    if (!r3) return -1;
    okp = bson_scan(r3, "ok", &t);
    ok = 0;
    if (okp) { if (t == 0x01) memcpy(&ok, okp, 8); else if (t == 0x10) ok = rd_i32(okp); }
    free(r3);
    return ok > 0.5 ? 0 : -1;
}

void *vader_mongo_connect(const char *dsn) {
    char host[128] = "127.0.0.1", db[128] = "test";
    int port = 27017;
    char user[128] = {0}, pass[128] = {0};
    const char *p = strstr(dsn, "://");
    p = p ? p + 3 : dsn;
    const char *at = strchr(p, '@');
    if (at) {
        const char *colon = memchr(p, ':', at - p);
        if (colon) {
            int ul = (int)(colon - p), pl = (int)(at - colon - 1);
            if (ul < 127) { memcpy(user, p, ul); user[ul] = 0; }
            if (pl < 127) { memcpy(pass, colon + 1, pl); pass[pl] = 0; }
        } else {
            int ul = (int)(at - p);
            if (ul < 127) { memcpy(user, p, ul); user[ul] = 0; }
        }
        p = at + 1;
    }
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
    if (user[0] && mongo_auth(m, user, pass) != 0) {  /* authenticate if credentials given */
        close(fd);
        free(m);
        return 0;
    }
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
    if (vader_json_as_float(vader_json_field(reply, "ok")) < 0.5)
        return vader_strdup("mongo: insert rejected (auth?)");
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

/* runs an aggregation `pipeline` (a JSON array of stages); returns the result array. */
void *vader_mongo_aggregate(void *mh, const char *coll, void *pipeline) {
    Mongo *m = mh;
    if (!m) return vader_json_array();
    void *cmd = vader_json_object();
    vader_json_set_str(cmd, "aggregate", coll);
    vader_json_set(cmd, "pipeline", pipeline);
    vader_json_set(cmd, "cursor", vader_json_object());  /* empty cursor doc {} */
    vader_json_set_str(cmd, "$db", m->db);
    void *reply = op_msg(m, cmd);
    if (!reply) return vader_json_array();
    void *batch = vader_json_field(vader_json_field(reply, "cursor"), "firstBatch");
    if (vader_json_type(batch) != JT_ARR) return vader_json_array();
    return batch;
}

/* update documents matching `filter` by applying $set with `changes`. */
const char *vader_mongo_update(void *mh, const char *coll, void *filter, void *changes) {
    Mongo *m = mh;
    if (!m) return vader_strdup("mongo: not connected");
    void *cmd = vader_json_object();
    vader_json_set_str(cmd, "update", coll);
    void *updates = vader_json_array();
    void *one = vader_json_object();
    vader_json_set(one, "q", filter);
    void *set = vader_json_object();
    vader_json_set(set, "$set", changes);
    vader_json_set(one, "u", set);
    vader_json_set_bool(one, "multi", 1);
    vader_json_add(updates, one);
    vader_json_set(cmd, "updates", updates);
    vader_json_set_str(cmd, "$db", m->db);
    void *reply = op_msg(m, cmd);
    if (!reply) return vader_strdup("mongo: request failed");
    if (vader_json_as_float(vader_json_field(reply, "ok")) < 0.5)
        return vader_strdup("mongo: update rejected (auth?)");
    return 0;
}

/* delete every document matching `filter` (limit 0 = all). */
const char *vader_mongo_delete(void *mh, const char *coll, void *filter) {
    Mongo *m = mh;
    if (!m) return vader_strdup("mongo: not connected");
    void *cmd = vader_json_object();
    vader_json_set_str(cmd, "delete", coll);
    void *deletes = vader_json_array();
    void *one = vader_json_object();
    vader_json_set(one, "q", filter);
    vader_json_set_int(one, "limit", 0);
    vader_json_add(deletes, one);
    vader_json_set(cmd, "deletes", deletes);
    vader_json_set_str(cmd, "$db", m->db);
    void *reply = op_msg(m, cmd);
    if (!reply) return vader_strdup("mongo: request failed");
    if (vader_json_as_float(vader_json_field(reply, "ok")) < 0.5)
        return vader_strdup("mongo: delete rejected (auth?)");
    return 0;
}

void vader_mongo_close(void *mh) {
    Mongo *m = mh;
    if (m) {
        if (m->fd >= 0) close(m->fd);
        free(m);
    }
}
