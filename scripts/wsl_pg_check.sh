#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== clang -c vader_pg.c ==="
if clang -c -O2 -Wall runtime/vader_pg.c -o /tmp/vpg.o 2>/tmp/pg.log; then echo "PG compila OK"; else echo "PG FALHOU:"; cat /tmp/pg.log | head -30; fi
grep -E 'error:' /tmp/pg.log | head -20
echo "=== clang -c vader_db.c ==="
if clang -c -O2 -Iruntime/sqlite runtime/vader_db.c -o /tmp/vdb.o 2>/tmp/db.log; then echo "DB compila OK"; else echo "DB FALHOU:"; cat /tmp/db.log | head -30; fi
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -20
echo "=== fim ==="