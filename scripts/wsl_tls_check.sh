#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build + test ==="
cargo build 2>&1 | grep -E '^error' | head
cargo test --quiet 2>&1 | grep 'test result' | head -1
BIN=./target/debug/vader

echo "=== clang -c vader_pg.c (sem TLS) ==="
clang -c -O2 runtime/vader_pg.c -o /tmp/pg.o 2>/tmp/pg.log && echo "OK" || { echo FALHOU; grep error: /tmp/pg.log|head; }

echo "=== openssl-dev presente? ==="
if [ -f /usr/include/openssl/ssl.h ]; then echo "OPENSSL_HEADERS_OK"; else echo "NO_OPENSSL_DEV"; fi

echo "=== clang -c vader_pg.c (com TLS) ==="
if clang -c -O2 -DVADER_TLS runtime/vader_pg.c -o /tmp/pgtls.o 2>/tmp/pgtls.log; then echo "TLS compila OK"; else echo "TLS compila FALHOU:"; grep -E 'error:|fatal' /tmp/pgtls.log | head; fi

echo "=== regressão: SQLite ainda roda (após refatorar a IO do PG) ==="
rm -f /tmp/vader_demo.db
"$BIN" llvm examples/db_sqlite.vd 2>&1 | grep -vE 'emitted|compiled|linkando|compilando' | tail -3

echo "=== build com --tls (SQLite, linka openssl) ==="
rm -f /tmp/vader_demo.db
"$BIN" llvm --tls examples/db_sqlite.vd 2>&1 | grep -vE 'emitted|compiled' | tail -4