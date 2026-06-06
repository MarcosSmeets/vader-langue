/* Vader runtime Postgres client (wire protocol v3) — self-contained.
 *
 * TCP + StartupMessage + authentication (trust / cleartext / SCRAM-SHA-256) +
 * Simple Query Protocol, with results in text format. No libpq, no TLS
 * (v1: unencrypted connection — works for local/self-hosted PG without forced SSL).
 *
 * Includes its own SHA-256/HMAC/PBKDF2/base64 (public domain) for SCRAM.
 * No GC: result buffers leak, in line with the rest of the runtime. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>
#include <arpa/inet.h>

#ifdef VADER_TLS
#include <openssl/ssl.h>
#endif

/* ===================== SHA-256 (public domain, Brad Conte) ================ */
typedef struct {
    unsigned char data[64];
    unsigned int datalen;
    unsigned long long bitlen;
    unsigned int state[8];
} SHA256_CTX;

#define ROTR(a, b) (((a) >> (b)) | ((a) << (32 - (b))))
#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (ROTR(x, 2) ^ ROTR(x, 13) ^ ROTR(x, 22))
#define EP1(x) (ROTR(x, 6) ^ ROTR(x, 11) ^ ROTR(x, 25))
#define SIG0(x) (ROTR(x, 7) ^ ROTR(x, 18) ^ ((x) >> 3))
#define SIG1(x) (ROTR(x, 17) ^ ROTR(x, 19) ^ ((x) >> 10))

static const unsigned int sha256_k[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
    0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};

static void sha256_transform(SHA256_CTX *ctx, const unsigned char *data) {
    unsigned int a, b, c, d, e, f, g, h, i, j, t1, t2, m[64];
    for (i = 0, j = 0; i < 16; ++i, j += 4)
        m[i] = (data[j] << 24) | (data[j + 1] << 16) | (data[j + 2] << 8) | data[j + 3];
    for (; i < 64; ++i)
        m[i] = SIG1(m[i - 2]) + m[i - 7] + SIG0(m[i - 15]) + m[i - 16];
    a = ctx->state[0]; b = ctx->state[1]; c = ctx->state[2]; d = ctx->state[3];
    e = ctx->state[4]; f = ctx->state[5]; g = ctx->state[6]; h = ctx->state[7];
    for (i = 0; i < 64; ++i) {
        t1 = h + EP1(e) + CH(e, f, g) + sha256_k[i] + m[i];
        t2 = EP0(a) + MAJ(a, b, c);
        h = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
    }
    ctx->state[0] += a; ctx->state[1] += b; ctx->state[2] += c; ctx->state[3] += d;
    ctx->state[4] += e; ctx->state[5] += f; ctx->state[6] += g; ctx->state[7] += h;
}

static void sha256_init(SHA256_CTX *ctx) {
    ctx->datalen = 0; ctx->bitlen = 0;
    ctx->state[0] = 0x6a09e667; ctx->state[1] = 0xbb67ae85;
    ctx->state[2] = 0x3c6ef372; ctx->state[3] = 0xa54ff53a;
    ctx->state[4] = 0x510e527f; ctx->state[5] = 0x9b05688c;
    ctx->state[6] = 0x1f83d9ab; ctx->state[7] = 0x5be0cd19;
}

static void sha256_update(SHA256_CTX *ctx, const unsigned char *data, size_t len) {
    for (size_t i = 0; i < len; ++i) {
        ctx->data[ctx->datalen] = data[i];
        ctx->datalen++;
        if (ctx->datalen == 64) {
            sha256_transform(ctx, ctx->data);
            ctx->bitlen += 512;
            ctx->datalen = 0;
        }
    }
}

static void sha256_final(SHA256_CTX *ctx, unsigned char *hash) {
    unsigned int i = ctx->datalen;
    if (ctx->datalen < 56) {
        ctx->data[i++] = 0x80;
        while (i < 56) ctx->data[i++] = 0x00;
    } else {
        ctx->data[i++] = 0x80;
        while (i < 64) ctx->data[i++] = 0x00;
        sha256_transform(ctx, ctx->data);
        memset(ctx->data, 0, 56);
    }
    ctx->bitlen += (unsigned long long)ctx->datalen * 8;
    ctx->data[63] = ctx->bitlen; ctx->data[62] = ctx->bitlen >> 8;
    ctx->data[61] = ctx->bitlen >> 16; ctx->data[60] = ctx->bitlen >> 24;
    ctx->data[59] = ctx->bitlen >> 32; ctx->data[58] = ctx->bitlen >> 40;
    ctx->data[57] = ctx->bitlen >> 48; ctx->data[56] = ctx->bitlen >> 56;
    sha256_transform(ctx, ctx->data);
    for (i = 0; i < 4; ++i) {
        for (int j = 0; j < 8; ++j)
            hash[i + j * 4] = (ctx->state[j] >> (24 - i * 8)) & 0xff;
    }
}

static void sha256(const unsigned char *data, size_t len, unsigned char out[32]) {
    SHA256_CTX c;
    sha256_init(&c);
    sha256_update(&c, data, len);
    sha256_final(&c, out);
}

/* HMAC-SHA256 */
static void hmac_sha256(const unsigned char *key, int keylen,
                        const unsigned char *msg, int msglen, unsigned char out[32]) {
    unsigned char k[64], ipad[64], opad[64], inner[32];
    memset(k, 0, 64);
    if (keylen > 64) {
        sha256(key, keylen, k);
    } else {
        memcpy(k, key, keylen);
    }
    for (int i = 0; i < 64; i++) {
        ipad[i] = k[i] ^ 0x36;
        opad[i] = k[i] ^ 0x5c;
    }
    SHA256_CTX c;
    sha256_init(&c);
    sha256_update(&c, ipad, 64);
    sha256_update(&c, msg, msglen);
    sha256_final(&c, inner);
    sha256_init(&c);
    sha256_update(&c, opad, 64);
    sha256_update(&c, inner, 32);
    sha256_final(&c, out);
}

/* PBKDF2-HMAC-SHA256, 1 output block (32 bytes) */
static void pbkdf2_sha256(const char *pass, int passlen, const unsigned char *salt,
                          int saltlen, int iter, unsigned char out[32]) {
    unsigned char buf[128], u[32], t[32];
    int i;
    if (saltlen > 120) saltlen = 120;
    memcpy(buf, salt, saltlen);
    buf[saltlen] = 0; buf[saltlen + 1] = 0; buf[saltlen + 2] = 0; buf[saltlen + 3] = 1;
    hmac_sha256((const unsigned char *)pass, passlen, buf, saltlen + 4, u);
    memcpy(t, u, 32);
    for (i = 1; i < iter; i++) {
        hmac_sha256((const unsigned char *)pass, passlen, u, 32, u);
        for (int j = 0; j < 32; j++) t[j] ^= u[j];
    }
    memcpy(out, t, 32);
}

/* ===================== base64 ============================================= */
static const char B64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static char *b64_encode(const unsigned char *in, int len) {
    char *out = malloc(((len + 2) / 3) * 4 + 1);
    int o = 0;
    for (int i = 0; i < len; i += 3) {
        int n = in[i] << 16;
        if (i + 1 < len) n |= in[i + 1] << 8;
        if (i + 2 < len) n |= in[i + 2];
        out[o++] = B64[(n >> 18) & 63];
        out[o++] = B64[(n >> 12) & 63];
        out[o++] = (i + 1 < len) ? B64[(n >> 6) & 63] : '=';
        out[o++] = (i + 2 < len) ? B64[n & 63] : '=';
    }
    out[o] = 0;
    return out;
}

static int b64_val(char c) {
    if (c >= 'A' && c <= 'Z') return c - 'A';
    if (c >= 'a' && c <= 'z') return c - 'a' + 26;
    if (c >= '0' && c <= '9') return c - '0' + 52;
    if (c == '+') return 62;
    if (c == '/') return 63;
    return -1;
}

static int b64_decode(const char *in, int inlen, unsigned char *out) {
    int o = 0, bits = 0, acc = 0;
    for (int i = 0; i < inlen; i++) {
        int v = b64_val(in[i]);
        if (v < 0) continue;
        acc = (acc << 6) | v;
        bits += 6;
        if (bits >= 8) {
            bits -= 8;
            out[o++] = (acc >> bits) & 0xff;
        }
    }
    return o;
}

/* ===================== socket helpers ===================================== */
static int read_n(int fd, unsigned char *buf, int n) {
    int got = 0;
    while (got < n) {
        int r = read(fd, buf + got, n - got);
        if (r <= 0) return -1;
        got += r;
    }
    return 0;
}

static int write_all(int fd, const unsigned char *buf, int n) {
    int sent = 0;
    while (sent < n) {
        int w = write(fd, buf + sent, n - sent);
        if (w <= 0) return -1;
        sent += w;
    }
    return 0;
}

/* put int32 big-endian */
static void put32(unsigned char *p, unsigned int v) {
    p[0] = v >> 24; p[1] = v >> 16; p[2] = v >> 8; p[3] = v;
}
static unsigned int get32(const unsigned char *p) {
    return ((unsigned int)p[0] << 24) | (p[1] << 16) | (p[2] << 8) | p[3];
}

/* ===================== connection, structs and IO ========================= */
typedef struct {
    int fd;
    void *ssl; /* SSL* when TLS is active (NULL = plaintext) */
    char err[256];
} PgConn;

typedef struct {
    int ncols, nrows, cur;
    char ***cells; /* cells[row][col] (text, or NULL) */
} PgRows;

/* IO that goes through TLS when active, otherwise straight to the socket. */
static int io_read(PgConn *c, unsigned char *buf, int n) {
#ifdef VADER_TLS
    if (c->ssl) {
        int got = 0;
        while (got < n) {
            int r = SSL_read((SSL *)c->ssl, buf + got, n - got);
            if (r <= 0) return -1;
            got += r;
        }
        return 0;
    }
#endif
    return read_n(c->fd, buf, n);
}
static int io_write(PgConn *c, const unsigned char *buf, int n) {
#ifdef VADER_TLS
    if (c->ssl) {
        int s = 0;
        while (s < n) {
            int w = SSL_write((SSL *)c->ssl, buf + s, n - s);
            if (w <= 0) return -1;
            s += w;
        }
        return 0;
    }
#endif
    return write_all(c->fd, buf, n);
}

/* Reads a message from the backend: 1 type byte + int32 len. Allocates the body. */
static int pg_read_msg(PgConn *c, char *type, unsigned char **body, int *bodylen) {
    unsigned char hdr[5];
    if (io_read(c, hdr, 5) < 0) return -1;
    *type = hdr[0];
    int len = get32(hdr + 1); /* includes the 4 bytes of len itself */
    *bodylen = len - 4;
    if (*bodylen < 0) return -1;
    *body = malloc(*bodylen > 0 ? *bodylen : 1);
    if (*bodylen > 0 && io_read(c, *body, *bodylen) < 0) return -1;
    return 0;
}

/* sends a typed message (type + len + body) */
static int pg_send(PgConn *c, char type, const unsigned char *body, int bodylen) {
    unsigned char hdr[5];
    hdr[0] = type;
    put32(hdr + 1, bodylen + 4);
    if (io_write(c, hdr, 5) < 0) return -1;
    if (bodylen > 0 && io_write(c, body, bodylen) < 0) return -1;
    return 0;
}

/* random nonce (base64-safe alphanumeric) from /dev/urandom */
static void gen_nonce(char *out, int n) {
    unsigned char raw[64];
    int fd = open("/dev/urandom", 0);
    if (fd >= 0) {
        read_n(fd, raw, n);
        close(fd);
    } else {
        for (int i = 0; i < n; i++) raw[i] = (unsigned char)(i * 7 + 13);
    }
    for (int i = 0; i < n; i++) {
        out[i] = B64[raw[i] % 62]; /* alphanumeric only */
    }
    out[n] = 0;
}

/* parse of DSN postgres://user:pass@host:port/db */
static void dsn_parse(const char *dsn, char *user, char *pass, char *host,
                      int *port, char *db) {
    user[0] = pass[0] = host[0] = db[0] = 0;
    *port = 5432;
    const char *p = strstr(dsn, "://");
    p = p ? p + 3 : dsn;
    /* user:pass@ */
    const char *at = strrchr(p, '@');
    if (at) {
        const char *colon = memchr(p, ':', at - p);
        if (colon) {
            memcpy(user, p, colon - p); user[colon - p] = 0;
            memcpy(pass, colon + 1, at - colon - 1); pass[at - colon - 1] = 0;
        } else {
            memcpy(user, p, at - p); user[at - p] = 0;
        }
        p = at + 1;
    }
    /* host:port/db (strips query string) */
    const char *slash = strchr(p, '/');
    const char *hostend = slash ? slash : p + strlen(p);
    const char *colon = memchr(p, ':', hostend - p);
    if (colon) {
        memcpy(host, p, colon - p); host[colon - p] = 0;
        *port = atoi(colon + 1);
    } else {
        memcpy(host, p, hostend - p); host[hostend - p] = 0;
    }
    if (slash) {
        const char *q = strchr(slash + 1, '?');
        const char *dbend = q ? q : slash + 1 + strlen(slash + 1);
        memcpy(db, slash + 1, dbend - slash - 1); db[dbend - slash - 1] = 0;
    }
    if (!host[0]) strcpy(host, "127.0.0.1");
    if (!db[0]) strcpy(db, user);
}

/* SCRAM-SHA-256 authentication. Returns 0 on success. */
static int scram_auth(PgConn *c, const char *user, const char *pass) {
    (void)user;
    char cnonce[33];
    gen_nonce(cnonce, 24);
    char client_first_bare[128];
    snprintf(client_first_bare, sizeof(client_first_bare), "n=,r=%s", cnonce);

    /* SASLInitialResponse: mechanism + int32 len + client-first ("n,," + bare) */
    char gs2[160];
    snprintf(gs2, sizeof(gs2), "n,,%s", client_first_bare);
    const char *mech = "SCRAM-SHA-256";
    int initlen = strlen(mech) + 1 + 4 + strlen(gs2);
    unsigned char *init = malloc(initlen);
    int o = 0;
    memcpy(init + o, mech, strlen(mech) + 1); o += strlen(mech) + 1;
    put32(init + o, strlen(gs2)); o += 4;
    memcpy(init + o, gs2, strlen(gs2)); o += strlen(gs2);
    pg_send(c,'p', init, initlen);
    free(init);

    /* expects 'R' 11 (SASLContinue) with server-first */
    char type; unsigned char *body; int blen;
    if (pg_read_msg(c,&type, &body, &blen) < 0) return -1;
    if (type == 'E') { strncpy(c->err, "auth error (server-first)", 255); return -1; }
    if (type != 'R' || get32(body) != 11) return -1;
    char server_first[256];
    int sflen = blen - 4;
    if (sflen > 255) sflen = 255;
    memcpy(server_first, body + 4, sflen); server_first[sflen] = 0;
    free(body);

    /* parse r=, s=, i= */
    char rnonce[128] = {0}, salt_b64[128] = {0};
    int iter = 4096;
    {
        char *r = strstr(server_first, "r="), *s = strstr(server_first, "s="),
             *it = strstr(server_first, "i=");
        if (!r || !s || !it) return -1;
        sscanf(r + 2, "%127[^,]", rnonce);
        sscanf(s + 2, "%127[^,]", salt_b64);
        iter = atoi(it + 2);
    }
    unsigned char salt[128];
    int saltlen = b64_decode(salt_b64, strlen(salt_b64), salt);

    /* SaltedPassword = PBKDF2(pass, salt, iter) */
    unsigned char salted[32], client_key[32], stored_key[32], client_sig[32], proof[32];
    pbkdf2_sha256(pass, strlen(pass), salt, saltlen, iter, salted);
    hmac_sha256(salted, 32, (const unsigned char *)"Client Key", 10, client_key);
    sha256(client_key, 32, stored_key);

    char client_final_noproof[160];
    snprintf(client_final_noproof, sizeof(client_final_noproof), "c=biws,r=%s", rnonce);
    char auth_msg[512];
    snprintf(auth_msg, sizeof(auth_msg), "%s,%s,%s", client_first_bare, server_first,
             client_final_noproof);
    hmac_sha256(stored_key, 32, (const unsigned char *)auth_msg, strlen(auth_msg), client_sig);
    for (int i = 0; i < 32; i++) proof[i] = client_key[i] ^ client_sig[i];
    char *proof_b64 = b64_encode(proof, 32);

    char client_final[256];
    snprintf(client_final, sizeof(client_final), "%s,p=%s", client_final_noproof, proof_b64);
    free(proof_b64);
    pg_send(c,'p', (unsigned char *)client_final, strlen(client_final));

    /* expects 'R' 12 (SASLFinal) and then 'R' 0 (AuthOk) */
    if (pg_read_msg(c,&type, &body, &blen) < 0) return -1;
    if (type == 'E') { strncpy(c->err, "SCRAM auth rejected (password?)", 255); return -1; }
    if (type != 'R') return -1;
    unsigned int code = get32(body);
    free(body);
    if (code == 12) {
        if (pg_read_msg(c,&type, &body, &blen) < 0) return -1;
        code = (type == 'R') ? get32(body) : 999;
        free(body);
    }
    return code == 0 ? 0 : -1;
}

/* reads from the backend until ReadyForQuery after the auth phase (ParameterStatus etc.) */
static int pg_consume_until_ready(PgConn *c) {
    for (;;) {
        char type; unsigned char *body; int blen;
        if (pg_read_msg(c,&type, &body, &blen) < 0) return -1;
        if (type == 'E') {
            strncpy(c->err, "server error after auth", sizeof(c->err) - 1);
            free(body);
            return -1;
        }
        free(body);
        if (type == 'Z') return 0; /* ReadyForQuery */
    }
}

/* opens connection: TCP + startup + auth. Returns handle or NULL. */
PgConn *vader_pg_connect(const char *dsn) {
    char user[128], pass[128], host[128], db[128];
    int port;
    dsn_parse(dsn, user, pass, host, &port, db);

    struct addrinfo hints, *res;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    char portstr[16];
    snprintf(portstr, sizeof(portstr), "%d", port);
    if (getaddrinfo(host, portstr, &hints, &res) != 0) return 0;
    int fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd < 0 || connect(fd, res->ai_addr, res->ai_addrlen) < 0) {
        freeaddrinfo(res);
        return 0;
    }
    freeaddrinfo(res);

    PgConn *c = calloc(1, sizeof(PgConn));
    c->fd = fd;

#ifdef VADER_TLS
    /* TLS negotiation: sends SSLRequest (plaintext). 'S' = accepts -> handshake. */
    {
        unsigned char req[8];
        put32(req, 8);
        put32(req + 4, 80877103); /* SSLRequest magic code */
        if (write_all(fd, req, 8) == 0) {
            unsigned char resp = 0;
            if (read_n(fd, &resp, 1) == 0 && resp == 'S') {
                SSL_CTX *ctx = SSL_CTX_new(TLS_client_method());
                if (ctx) {
                    SSL *ssl = SSL_new(ctx);
                    SSL_set_fd(ssl, fd);
                    if (SSL_connect(ssl) == 1) {
                        c->ssl = ssl; /* v1: no certificate verification */
                    } else {
                        strcpy(c->err, "TLS handshake failed");
                        c->fd = -1;
                        return c;
                    }
                }
            }
            /* 'N' = server without TLS: continues in plaintext (sslmode=prefer style) */
        }
    }
#endif

    /* StartupMessage: int32 len, int32 196608, "user"\0 user \0 "database"\0 db \0 \0 */
    unsigned char start[512];
    int o = 8;
    o += sprintf((char *)start + o, "user") + 1;
    o += sprintf((char *)start + o, "%s", user) + 1;
    o += sprintf((char *)start + o, "database") + 1;
    o += sprintf((char *)start + o, "%s", db) + 1;
    start[o++] = 0;
    put32(start, o);
    put32(start + 4, 196608);
    if (io_write(c, start, o) < 0) { c->fd = -1; return c; }

    /* authentication loop */
    for (;;) {
        char type; unsigned char *body; int blen;
        if (pg_read_msg(c, &type, &body, &blen) < 0) { strcpy(c->err, "connection dropped"); c->fd = -1; return c; }
        if (type == 'E') { strcpy(c->err, "server error at startup"); free(body); c->fd = -1; return c; }
        if (type != 'R') { free(body); continue; }
        unsigned int code = get32(body);
        if (code == 0) { free(body); break; }               /* AuthenticationOk */
        if (code == 3) {                                     /* cleartext */
            free(body);
            pg_send(c, 'p', (unsigned char *)pass, strlen(pass) + 1);
            continue;
        }
        if (code == 10) {                                    /* SASL / SCRAM */
            free(body);
            if (scram_auth(c, user, pass) < 0) { c->fd = -1; return c; }
            continue;
        }
        /* MD5 (5) and others not supported in v1 */
        free(body);
        snprintf(c->err, sizeof(c->err), "auth method %u not supported (use trust/password/scram)", code);
        c->fd = -1;
        return c;
    }
    if (pg_consume_until_ready(c) < 0) { c->fd = -1; return c; }
    return c;
}

const char *vader_pg_error(PgConn *c) {
    return (c && c->fd < 0 && c->err[0]) ? strdup(c->err) : 0;
}

/* extracts the message from an ErrorResponse ('E'): type+string\0 fields, end at \0 */
static char *pg_error_text(const unsigned char *body, int blen) {
    int i = 0;
    while (i < blen && body[i] != 0) {
        char field = body[i++];
        const char *val = (const char *)body + i;
        int len = strlen(val);
        if (field == 'M') return strdup(val); /* Message */
        i += len + 1;
    }
    return strdup("Postgres error");
}

/* runs a query via Simple Query; buffers rows as text. */
static PgRows *pg_run(PgConn *c, const char *sql, char **errout) {
    *errout = 0;
    if (!c || c->fd < 0) { *errout = strdup("invalid connection"); return 0; }
    int qlen = strlen(sql) + 1;
    pg_send(c,'Q', (const unsigned char *)sql, qlen);

    PgRows *rows = calloc(1, sizeof(PgRows));
    rows->cur = -1;
    int cap = 0;
    for (;;) {
        char type; unsigned char *body; int blen;
        if (pg_read_msg(c,&type, &body, &blen) < 0) {
            *errout = strdup("connection dropped during the query");
            return rows;
        }
        if (type == 'T') { /* RowDescription */
            rows->ncols = (body[0] << 8) | body[1];
        } else if (type == 'D') { /* DataRow */
            int nc = (body[0] << 8) | body[1];
            char **row = calloc(nc, sizeof(char *));
            int p = 2;
            for (int col = 0; col < nc; col++) {
                int len = (int)get32(body + p);
                p += 4;
                if (len < 0) { row[col] = 0; }
                else {
                    row[col] = malloc(len + 1);
                    memcpy(row[col], body + p, len);
                    row[col][len] = 0;
                    p += len;
                }
            }
            if (rows->nrows >= cap) {
                cap = cap ? cap * 2 : 16;
                rows->cells = realloc(rows->cells, cap * sizeof(char **));
            }
            rows->cells[rows->nrows++] = row;
        } else if (type == 'E') {
            *errout = pg_error_text(body, blen);
            free(body);
            /* drains until ReadyForQuery */
            for (;;) {
                char t2; unsigned char *b2; int l2;
                if (pg_read_msg(c,&t2, &b2, &l2) < 0) break;
                free(b2);
                if (t2 == 'Z') break;
            }
            return rows;
        } else if (type == 'Z') {
            free(body);
            break;
        }
        free(body);
    }
    return rows;
}

/* API exposed to Vader (same shape as SQLite). */
const char *vader_pg_exec(PgConn *c, const char *sql) {
    char *err;
    pg_run(c, sql, &err);
    return err; /* NULL on success */
}

PgRows *vader_pg_query(PgConn *c, const char *sql) {
    char *err;
    PgRows *r = pg_run(c, sql, &err);
    return r;
}

int vader_pg_next(PgRows *r) {
    if (!r) return 0;
    r->cur++;
    return r->cur < r->nrows ? 1 : 0;
}

const char *vader_pg_text(PgRows *r, int col) {
    if (!r || r->cur < 0 || r->cur >= r->nrows || col >= r->ncols) return strdup("");
    char *v = r->cells[r->cur][col];
    return strdup(v ? v : "");
}

void vader_pg_close(PgConn *c) {
    if (c && c->fd >= 0) {
        unsigned char term[5] = {'X', 0, 0, 0, 4};
        io_write(c, term, 5);
#ifdef VADER_TLS
        if (c->ssl) SSL_free((SSL *)c->ssl);
#endif
        close(c->fd);
        c->fd = -1;
    }
}
