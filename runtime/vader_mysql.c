/* Vader runtime MySQL/MariaDB client (native protocol) — self-contained.
 *
 * TCP + handshake v10 + COM_QUERY + result set parsing (text). No libmysqlclient.
 *
 * Auth: `mysql_native_password` (SHA-1) and `caching_sha2_password` (MySQL 8 default,
 * SHA-256 scramble). The caching_sha2 fast path needs no extra crypto; full auth (cold
 * cache) sends the RSA-encrypted password using the server's public key, which requires
 * OpenSSL — available only when built with `--tls`. Without it, only the fast path
 * (cached credential) works for caching_sha2.
 *
 * Includes its own SHA-1 (public domain); SHA-256 comes from vader_scram.c. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>

#ifdef VADER_TLS
#include <openssl/rsa.h>
#include <openssl/pem.h>
#include <openssl/bio.h>
#endif

/* SHA-256 (vader_scram.c) for the caching_sha2_password scramble */
extern void vader_scram_sha256(const unsigned char *, int, unsigned char[32]);

/* ===================== SHA-1 (public domain) ============================== */
typedef struct {
    unsigned int state[5];
    unsigned long long count;
    unsigned char buffer[64];
} SHA1_CTX;

static void sha1_transform(unsigned int state[5], const unsigned char buffer[64]) {
    unsigned int blk[80];
    for (int i = 0; i < 16; i++)
        blk[i] = (buffer[i * 4] << 24) | (buffer[i * 4 + 1] << 16) |
                 (buffer[i * 4 + 2] << 8) | buffer[i * 4 + 3];
    for (int i = 16; i < 80; i++) {
        unsigned int w = blk[i - 3] ^ blk[i - 8] ^ blk[i - 14] ^ blk[i - 16];
        blk[i] = (w << 1) | (w >> 31);
    }
    unsigned int a = state[0], b = state[1], c = state[2], d = state[3], e = state[4];
    int i;
    unsigned int *bl = blk;
#define STEP(f, k) \
    do { unsigned int tmp = ((a << 5) | (a >> 27)) + (f) + e + (k) + *bl++; \
         e = d; d = c; c = (b << 30) | (b >> 2); b = a; a = tmp; } while (0)
    for (i = 0; i < 20; i++) STEP((b & (c ^ d)) ^ d, 0x5A827999);
    for (i = 0; i < 20; i++) STEP(b ^ c ^ d, 0x6ED9EBA1);
    for (i = 0; i < 20; i++) STEP((b & c) | (d & (b | c)), 0x8F1BBCDC);
    for (i = 0; i < 20; i++) STEP(b ^ c ^ d, 0xCA62C1D6);
#undef STEP
    state[0] += a; state[1] += b; state[2] += c; state[3] += d; state[4] += e;
}

static void sha1_init(SHA1_CTX *c) {
    c->state[0] = 0x67452301; c->state[1] = 0xEFCDAB89; c->state[2] = 0x98BADCFE;
    c->state[3] = 0x10325476; c->state[4] = 0xC3D2E1F0; c->count = 0;
}
static void sha1_update(SHA1_CTX *c, const unsigned char *data, size_t len) {
    size_t i = (size_t)((c->count >> 3) & 63);
    c->count += (unsigned long long)len << 3;
    for (size_t k = 0; k < len; k++) {
        c->buffer[i++] = data[k];
        if (i == 64) { sha1_transform(c->state, c->buffer); i = 0; }
    }
}
static void sha1_final(SHA1_CTX *c, unsigned char out[20]) {
    unsigned char finalcount[8];
    for (int i = 0; i < 8; i++)
        finalcount[i] = (unsigned char)((c->count >> ((7 - i) * 8)) & 255);
    unsigned char ch = 0x80;
    sha1_update(c, &ch, 1);
    unsigned char z = 0;
    while (((c->count >> 3) & 63) != 56) sha1_update(c, &z, 1);
    sha1_update(c, finalcount, 8);
    for (int i = 0; i < 20; i++)
        out[i] = (unsigned char)((c->state[i >> 2] >> ((3 - (i & 3)) * 8)) & 255);
}
static void sha1(const unsigned char *d, size_t n, unsigned char out[20]) {
    SHA1_CTX c; sha1_init(&c); sha1_update(&c, d, n); sha1_final(&c, out);
}

/* ===================== socket + packets =================================== */
static int my_read_n(int fd, unsigned char *buf, int n) {
    int got = 0;
    while (got < n) {
        int r = read(fd, buf + got, n - got);
        if (r <= 0) return -1;
        got += r;
    }
    return 0;
}
static int my_write_all(int fd, const unsigned char *buf, int n) {
    int s = 0;
    while (s < n) {
        int w = write(fd, buf + s, n - s);
        if (w <= 0) return -1;
        s += w;
    }
    return 0;
}

/* reads a MySQL packet: 3 len bytes (LE) + 1 seq byte. Allocates the payload. */
static unsigned char *my_read_packet(int fd, int *outlen, int *seq) {
    unsigned char hdr[4];
    if (my_read_n(fd, hdr, 4) < 0) return 0;
    int len = hdr[0] | (hdr[1] << 8) | (hdr[2] << 16);
    *seq = hdr[3];
    unsigned char *p = malloc(len > 0 ? len : 1);
    if (len > 0 && my_read_n(fd, p, len) < 0) { free(p); return 0; }
    *outlen = len;
    return p;
}
static int my_write_packet(int fd, const unsigned char *payload, int len, int seq) {
    unsigned char hdr[4];
    hdr[0] = len & 0xff; hdr[1] = (len >> 8) & 0xff; hdr[2] = (len >> 16) & 0xff; hdr[3] = seq;
    if (my_write_all(fd, hdr, 4) < 0) return -1;
    return my_write_all(fd, payload, len);
}

/* length-encoded integer: returns the value and advances *p */
static unsigned long long lenenc_int(const unsigned char **p) {
    unsigned char b = *(*p)++;
    if (b < 0xfb) return b;
    if (b == 0xfc) { unsigned long long v = (*p)[0] | ((*p)[1] << 8); *p += 2; return v; }
    if (b == 0xfd) { unsigned long long v = (*p)[0] | ((*p)[1] << 8) | ((*p)[2] << 16); *p += 3; return v; }
    unsigned long long v = 0;
    for (int i = 0; i < 8; i++) v |= (unsigned long long)(*p)[i] << (i * 8);
    *p += 8;
    return v;
}

typedef struct {
    int fd;
    char err[256];
} MyConn;

typedef struct {
    int ncols, nrows, cur;
    char ***cells;
} MyRows;

static void my_dsn_parse(const char *dsn, char *user, char *pass, char *host,
                         int *port, char *db) {
    user[0] = pass[0] = host[0] = db[0] = 0;
    *port = 3306;
    const char *p = strstr(dsn, "://");
    p = p ? p + 3 : dsn;
    const char *at = strrchr(p, '@');
    if (at) {
        const char *colon = memchr(p, ':', at - p);
        if (colon) {
            memcpy(user, p, colon - p); user[colon - p] = 0;
            memcpy(pass, colon + 1, at - colon - 1); pass[at - colon - 1] = 0;
        } else { memcpy(user, p, at - p); user[at - p] = 0; }
        p = at + 1;
    }
    const char *slash = strchr(p, '/');
    const char *hostend = slash ? slash : p + strlen(p);
    const char *colon = memchr(p, ':', hostend - p);
    if (colon) { memcpy(host, p, colon - p); host[colon - p] = 0; *port = atoi(colon + 1); }
    else { memcpy(host, p, hostend - p); host[hostend - p] = 0; }
    if (slash) {
        const char *q = strchr(slash + 1, '?');
        const char *dbend = q ? q : slash + 1 + strlen(slash + 1);
        memcpy(db, slash + 1, dbend - slash - 1); db[dbend - slash - 1] = 0;
    }
    if (!host[0]) strcpy(host, "127.0.0.1");
}

/* mysql_native_password scramble: SHA1(pwd) XOR SHA1(salt + SHA1(SHA1(pwd))) */
static void native_scramble(const char *pass, const unsigned char *salt, unsigned char out[20]) {
    unsigned char h1[20], h2[20], h3[20], cat[40];
    sha1((const unsigned char *)pass, strlen(pass), h1);
    sha1(h1, 20, h2);
    memcpy(cat, salt, 20);
    memcpy(cat + 20, h2, 20);
    sha1(cat, 40, h3);
    for (int i = 0; i < 20; i++) out[i] = h1[i] ^ h3[i];
}

/* caching_sha2_password scramble: SHA256(pwd) XOR SHA256(SHA256(SHA256(pwd)) + salt) */
static void caching_sha2_scramble(const char *pass, const unsigned char *salt, unsigned char out[32]) {
    unsigned char h1[32], h2[32], h3[32], cat[52];
    vader_scram_sha256((const unsigned char *)pass, strlen(pass), h1);
    vader_scram_sha256(h1, 32, h2);
    memcpy(cat, h2, 32);
    memcpy(cat + 32, salt, 20);
    vader_scram_sha256(cat, 52, h3);
    for (int i = 0; i < 32; i++) out[i] = h1[i] ^ h3[i];
}

#ifdef VADER_TLS
/* RSA-OAEP encrypt `in` with the server's PEM public key; returns ciphertext length. */
static int rsa_oaep_encrypt(const char *pem, int pemlen, const unsigned char *in, int inlen,
                            unsigned char *out) {
    BIO *bio = BIO_new_mem_buf(pem, pemlen);
    if (!bio) return -1;
    RSA *rsa = PEM_read_bio_RSA_PUBKEY(bio, NULL, NULL, NULL);
    BIO_free(bio);
    if (!rsa) return -1;
    int n = RSA_public_encrypt(inlen, in, out, rsa, RSA_PKCS1_OAEP_PADDING);
    RSA_free(rsa);
    return n;
}
#endif

MyConn *vader_my_connect(const char *dsn) {
    char user[128], pass[128], host[128], db[128];
    int port;
    my_dsn_parse(dsn, user, pass, host, &port, db);

    struct addrinfo hints, *res;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    char ps[16];
    snprintf(ps, sizeof(ps), "%d", port);
    if (getaddrinfo(host, ps, &hints, &res) != 0) return 0;
    int fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd < 0 || connect(fd, res->ai_addr, res->ai_addrlen) < 0) { freeaddrinfo(res); return 0; }
    freeaddrinfo(res);

    MyConn *c = calloc(1, sizeof(MyConn));
    c->fd = fd;

    /* initial server handshake */
    int len, seq;
    unsigned char *pkt = my_read_packet(fd, &len, &seq);
    if (!pkt) { c->fd = -1; return c; }
    /* parse: protocol(1), version\0, connid(4), salt1(8), filler(1), caps_low(2),
       charset(1), status(2), caps_high(2), authlen(1), reserved(10), salt2 */
    int i = 1;
    while (i < len && pkt[i]) i++;
    i++;            /* end of version */
    i += 4;         /* connection id */
    unsigned char salt[20];
    memcpy(salt, pkt + i, 8); i += 8;
    i += 1;         /* filler */
    i += 2;         /* caps low */
    i += 1;         /* charset */
    i += 2;         /* status */
    i += 2;         /* caps high */
    int authlen = pkt[i]; i += 1;
    i += 10;        /* reserved */
    int part2 = authlen > 8 ? authlen - 8 : 13;  /* auth-plugin-data-part-2 (>= 13) */
    if (part2 < 13) part2 = 13;
    int saltc = part2 - 1;
    if (saltc > 12) saltc = 12;
    memcpy(salt + 8, pkt + i, saltc);
    i += part2;
    char plugin[64] = "mysql_native_password";
    if (i < len && pkt[i]) {
        int pj = 0;
        while (i < len && pkt[i] && pj < 63) plugin[pj++] = pkt[i++];
        plugin[pj] = 0;
    }
    free(pkt);
    int use_sha2 = (strcmp(plugin, "caching_sha2_password") == 0);

    /* handshake response (protocol 41) */
    unsigned int caps = 0x1 | 0x200 | 0x2000 | 0x8000 | 0x80000 | 0x8; /* +CONNECT_WITH_DB */
    if (!db[0]) caps &= ~0x8u;
    unsigned char resp[512];
    int o = 0;
    resp[o++] = caps & 0xff; resp[o++] = (caps >> 8) & 0xff;
    resp[o++] = (caps >> 16) & 0xff; resp[o++] = (caps >> 24) & 0xff;
    resp[o++] = 0; resp[o++] = 0; resp[o++] = 0; resp[o++] = 1; /* max packet 16MB */
    resp[o++] = 33;                                            /* charset utf8 */
    memset(resp + o, 0, 23); o += 23;
    o += sprintf((char *)resp + o, "%s", user) + 1;
    if (pass[0]) {
        unsigned char scr[32];
        if (use_sha2) { caching_sha2_scramble(pass, salt, scr); resp[o++] = 32; memcpy(resp + o, scr, 32); o += 32; }
        else { native_scramble(pass, salt, scr); resp[o++] = 20; memcpy(resp + o, scr, 20); o += 20; }
    } else {
        resp[o++] = 0;
    }
    if (db[0]) o += sprintf((char *)resp + o, "%s", db) + 1;
    o += sprintf((char *)resp + o, "%s", use_sha2 ? "caching_sha2_password" : "mysql_native_password") + 1;
    my_write_packet(fd, resp, o, 1);

    /* auth result loop: OK / ERR / AuthSwitch(0xfe) / AuthMoreData(0x01, caching_sha2) */
    for (;;) {
        pkt = my_read_packet(fd, &len, &seq);
        if (!pkt) { c->fd = -1; if (!c->err[0]) snprintf(c->err, sizeof c->err, "no auth reply"); return c; }
        int rseq = seq + 1;
        unsigned char tag = pkt[0];
        if (tag == 0x00) { free(pkt); return c; }  /* OK packet */
        if (tag == 0xff) {
            int ecode = pkt[1] | (pkt[2] << 8);
            snprintf(c->err, sizeof(c->err), "auth failed (mysql error %d)", ecode);
            free(pkt); c->fd = -1; return c;
        }
        if (tag == 0xfe) {  /* AuthSwitchRequest: plugin\0 + salt */
            char sw[64]; int sj = 0, k = 1;
            while (k < len && pkt[k] && sj < 63) sw[sj++] = pkt[k++];
            sw[sj] = 0; k++;
            unsigned char nsalt[20];
            int avail = len - k; if (avail > 20) avail = 20; if (avail < 0) avail = 0;
            memcpy(nsalt, pkt + k, avail);
            free(pkt);
            memcpy(salt, nsalt, 20);
            use_sha2 = (strcmp(sw, "caching_sha2_password") == 0);
            unsigned char sc[32]; int sl;
            if (use_sha2) { caching_sha2_scramble(pass, salt, sc); sl = 32; }
            else { native_scramble(pass, salt, sc); sl = 20; }
            my_write_packet(fd, sc, sl, rseq);
            continue;
        }
        if (tag == 0x01) {  /* AuthMoreData (caching_sha2) */
            unsigned char st = pkt[1];
            free(pkt);
            if (st == 0x03) continue;  /* fast auth success -> OK follows */
            if (st == 0x04) {          /* full auth required */
#ifdef VADER_TLS
                unsigned char req = 0x02;  /* request the server's RSA public key */
                my_write_packet(fd, &req, 1, rseq);
                pkt = my_read_packet(fd, &len, &seq);
                if (!pkt) { c->fd = -1; return c; }
                rseq = seq + 1;
                const char *pem = (const char *)(pkt + 1);  /* 0x01 + PEM */
                int pemlen = len - 1;
                int plen = (int)strlen(pass) + 1;           /* include trailing NUL */
                unsigned char xored[256];
                for (int z = 0; z < plen && z < 256; z++)
                    xored[z] = (unsigned char)(z < plen - 1 ? pass[z] : 0) ^ salt[z % 20];
                unsigned char enc[512];
                int enclen = rsa_oaep_encrypt(pem, pemlen, xored, plen, enc);
                free(pkt);
                if (enclen <= 0) { snprintf(c->err, sizeof c->err, "RSA encrypt failed"); c->fd = -1; return c; }
                my_write_packet(fd, enc, enclen, rseq);
                continue;
#else
                snprintf(c->err, sizeof(c->err),
                         "caching_sha2 full auth needs --tls (or a cached credential)");
                c->fd = -1; return c;
#endif
            }
            snprintf(c->err, sizeof(c->err), "unexpected auth data 0x%02x", st);
            c->fd = -1; return c;
        }
        free(pkt);
        snprintf(c->err, sizeof(c->err), "unexpected auth packet 0x%02x", tag);
        c->fd = -1; return c;
    }
}

const char *vader_my_error(MyConn *c) {
    return (c && c->fd < 0 && c->err[0]) ? strdup(c->err) : 0;
}

static MyRows *my_run(MyConn *c, const char *sql, char **errout) {
    *errout = 0;
    if (!c || c->fd < 0) { *errout = strdup("invalid connection"); return 0; }
    int sqllen = strlen(sql);
    unsigned char *q = malloc(sqllen + 1);
    q[0] = 0x03; /* COM_QUERY */
    memcpy(q + 1, sql, sqllen);
    my_write_packet(c->fd, q, sqllen + 1, 0);
    free(q);

    MyRows *rows = calloc(1, sizeof(MyRows));
    rows->cur = -1;

    int len, seq;
    unsigned char *pkt = my_read_packet(c->fd, &len, &seq);
    if (!pkt) { *errout = strdup("connection dropped"); return rows; }
    if (pkt[0] == 0xff) { *errout = strdup("SQL error in MySQL"); free(pkt); return rows; }
    if (pkt[0] == 0x00) { free(pkt); return rows; } /* OK: no result set */
    const unsigned char *pp = pkt;
    rows->ncols = (int)lenenc_int(&pp);
    free(pkt);

    /* column definitions + EOF */
    for (int i = 0; i < rows->ncols; i++) { pkt = my_read_packet(c->fd, &len, &seq); free(pkt); }
    pkt = my_read_packet(c->fd, &len, &seq); /* EOF */
    free(pkt);

    /* rows until EOF */
    int cap = 0;
    for (;;) {
        pkt = my_read_packet(c->fd, &len, &seq);
        if (!pkt) break;
        if ((unsigned char)pkt[0] == 0xfe && len < 9) { free(pkt); break; } /* EOF */
        const unsigned char *p = pkt;
        char **row = calloc(rows->ncols, sizeof(char *));
        for (int col = 0; col < rows->ncols; col++) {
            if (*p == 0xfb) { row[col] = strdup(""); p++; continue; }
            unsigned long long l = lenenc_int(&p);
            row[col] = malloc(l + 1);
            memcpy(row[col], p, l);
            row[col][l] = 0;
            p += l;
        }
        if (rows->nrows >= cap) { cap = cap ? cap * 2 : 16; rows->cells = realloc(rows->cells, cap * sizeof(char **)); }
        rows->cells[rows->nrows++] = row;
        free(pkt);
    }
    return rows;
}

const char *vader_my_exec(MyConn *c, const char *sql) {
    char *err;
    my_run(c, sql, &err);
    return err;
}
void *vader_my_query(MyConn *c, const char *sql) {
    char *err;
    return my_run(c, sql, &err);
}
int vader_my_next(MyRows *r) {
    if (!r) return 0;
    r->cur++;
    return r->cur < r->nrows ? 1 : 0;
}
const char *vader_my_text(MyRows *r, int col) {
    if (!r || r->cur < 0 || r->cur >= r->nrows || col >= r->ncols) return strdup("");
    char *v = r->cells[r->cur][col];
    return strdup(v ? v : "");
}
void vader_my_close(MyConn *c) {
    if (c && c->fd >= 0) {
        unsigned char quit[1] = {0x01}; /* COM_QUIT */
        my_write_packet(c->fd, quit, 1, 0);
        close(c->fd);
        c->fd = -1;
    }
}
