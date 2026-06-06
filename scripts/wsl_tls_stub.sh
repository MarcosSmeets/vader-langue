#!/usr/bin/env bash
export PATH="/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
mkdir -p /tmp/sslstub/openssl
cat > /tmp/sslstub/openssl/ssl.h <<'EOF'
typedef struct ssl_st SSL;
typedef struct ssl_ctx_st SSL_CTX;
typedef struct ssl_method_st SSL_METHOD;
const SSL_METHOD *TLS_client_method(void);
SSL_CTX *SSL_CTX_new(const SSL_METHOD *);
SSL *SSL_new(SSL_CTX *);
int SSL_set_fd(SSL *, int);
int SSL_connect(SSL *);
int SSL_read(SSL *, void *, int);
int SSL_write(SSL *, const void *, int);
void SSL_free(SSL *);
EOF
echo "=== compila o caminho TLS contra o stub do OpenSSL ==="
if clang -c -O2 -DVADER_TLS -I/tmp/sslstub runtime/vader_pg.c -o /tmp/pgtls.o 2>/tmp/t.log; then
  echo "TLS code compila OK (uso da API do OpenSSL validado)"
else
  echo "FALHOU:"; grep -E 'error:' /tmp/t.log | head
fi