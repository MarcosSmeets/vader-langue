/* Shared SCRAM-SHA-256 crypto primitives (SHA-256/HMAC/PBKDF2/base64/nonce).
 * Public domain SHA-256 (Brad Conte). Used by the Mongo driver for auth. */
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>

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
    for (i = 0; i < 4; ++i)
        for (int j = 0; j < 8; ++j)
            hash[i + j * 4] = (ctx->state[j] >> (24 - i * 8)) & 0xff;
}

void vader_scram_sha256(const unsigned char *data, int len, unsigned char out[32]) {
    SHA256_CTX c;
    sha256_init(&c);
    sha256_update(&c, data, (size_t)len);
    sha256_final(&c, out);
}

void vader_scram_hmac(const unsigned char *key, int keylen, const unsigned char *msg,
                      int msglen, unsigned char out[32]) {
    unsigned char k[64], ipad[64], opad[64], inner[32];
    memset(k, 0, 64);
    if (keylen > 64) vader_scram_sha256(key, keylen, k);
    else memcpy(k, key, keylen);
    for (int i = 0; i < 64; i++) {
        ipad[i] = k[i] ^ 0x36;
        opad[i] = k[i] ^ 0x5c;
    }
    SHA256_CTX c;
    sha256_init(&c);
    sha256_update(&c, ipad, 64);
    sha256_update(&c, msg, (size_t)msglen);
    sha256_final(&c, inner);
    sha256_init(&c);
    sha256_update(&c, opad, 64);
    sha256_update(&c, inner, 32);
    sha256_final(&c, out);
}

void vader_scram_pbkdf2(const char *pass, int passlen, const unsigned char *salt,
                        int saltlen, int iter, unsigned char out[32]) {
    unsigned char buf[128], u[32], t[32];
    if (saltlen > 120) saltlen = 120;
    memcpy(buf, salt, saltlen);
    buf[saltlen] = 0; buf[saltlen + 1] = 0; buf[saltlen + 2] = 0; buf[saltlen + 3] = 1;
    vader_scram_hmac((const unsigned char *)pass, passlen, buf, saltlen + 4, u);
    memcpy(t, u, 32);
    for (int i = 1; i < iter; i++) {
        vader_scram_hmac((const unsigned char *)pass, passlen, u, 32, u);
        for (int j = 0; j < 32; j++) t[j] ^= u[j];
    }
    memcpy(out, t, 32);
}

static const char B64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

char *vader_scram_b64encode(const unsigned char *in, int len) {
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

int vader_scram_b64decode(const char *in, int inlen, unsigned char *out) {
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

/* random alphanumeric nonce from /dev/urandom */
void vader_scram_nonce(char *out, int n) {
    unsigned char raw[64];
    int fd = open("/dev/urandom", O_RDONLY);
    int got = 0;
    if (fd >= 0) {
        while (got < n) {
            int r = read(fd, raw + got, n - got);
            if (r <= 0) break;
            got += r;
        }
        close(fd);
    }
    for (int i = 0; i < n; i++) {
        unsigned char x = (i < got) ? raw[i] : (unsigned char)(i * 7 + 13);
        out[i] = B64[x % 62];
    }
    out[n] = 0;
}
