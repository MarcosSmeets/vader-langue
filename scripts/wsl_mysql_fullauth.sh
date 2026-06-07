#!/usr/bin/env bash
# Live-verify MySQL 8 caching_sha2 FULL auth (cold cache -> RSA) using the runtime
# OpenSSL .so + stub headers, so no libssl-dev install is needed.
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1

mkdir -p /tmp/sslstub/openssl
cat > /tmp/sslstub/openssl/rsa.h <<'EOF'
#ifndef STUB_RSA_H
#define STUB_RSA_H
typedef struct rsa_st RSA;
#define RSA_PKCS1_OAEP_PADDING 4
int RSA_public_encrypt(int flen, const unsigned char *from, unsigned char *to, RSA *rsa, int padding);
void RSA_free(RSA *rsa);
#endif
EOF
cat > /tmp/sslstub/openssl/bio.h <<'EOF'
#ifndef STUB_BIO_H
#define STUB_BIO_H
typedef struct bio_st BIO;
BIO *BIO_new_mem_buf(const void *buf, int len);
int BIO_free(BIO *a);
#endif
EOF
cat > /tmp/sslstub/openssl/pem.h <<'EOF'
#ifndef STUB_PEM_H
#define STUB_PEM_H
#include <openssl/rsa.h>
#include <openssl/bio.h>
RSA *PEM_read_bio_RSA_PUBKEY(BIO *bp, RSA **x, void *cb, void *u);
#endif
EOF

LIBCRYPTO=/usr/lib/x86_64-linux-gnu/libcrypto.so.3
echo "=== compile vader_mysql.c with VADER_TLS (stub headers) ==="
clang -O2 -DVADER_TLS -I/tmp/sslstub -c runtime/vader_mysql.c -o /tmp/vmy_tls.o 2>/tmp/c.log && echo "compiled OK" || { echo FAIL; grep -E 'error' /tmp/c.log | head; exit 1; }
clang -O2 -c runtime/vader_scram.c -o /tmp/vscram.o 2>/dev/null

cat > /tmp/mytls_test.c <<'EOF'
#include <stdio.h>
extern void *vader_my_connect(const char *dsn);
extern const char *vader_my_error(void *c);
extern const char *vader_my_exec(void *c, const char *sql);
extern void *vader_my_query(void *c, const char *sql);
extern int vader_my_next(void *r);
extern const char *vader_my_text(void *r, int col);
extern void vader_my_close(void *c);
int main(void){
    void *c = vader_my_connect("mysql://root:secret@127.0.0.1:3306/vaderdb");
    const char *e = vader_my_error(c);
    if (e) { printf("connect error: %s\n", e); return 1; }
    vader_my_exec(c, "DROP TABLE IF EXISTS t");
    vader_my_exec(c, "CREATE TABLE t (id INT, name VARCHAR(32))");
    vader_my_exec(c, "INSERT INTO t VALUES (1,'sha2-full-auth')");
    void *r = vader_my_query(c, "SELECT id, name FROM t");
    while (vader_my_next(r)) printf("  row: %s %s\n", vader_my_text(r,0), vader_my_text(r,1));
    vader_my_close(c);
    printf("OK: caching_sha2 full auth (cold cache) via RSA\n");
    return 0;
}
EOF
echo "=== link + run against MySQL 8 (root = caching_sha2, cold cache) ==="
clang -O2 /tmp/mytls_test.c /tmp/vmy_tls.o /tmp/vscram.o "$LIBCRYPTO" -o /tmp/mytls_test 2>/tmp/l.log && /tmp/mytls_test || { echo "LINK/RUN FAIL"; grep -E 'undefined|error' /tmp/l.log | head; }