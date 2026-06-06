#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -40
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|error\[' | head -10
echo "=== vader llvm db_sqlite (1st time: compiles SQLite) ==="
rm -f /tmp/vader_demo.db
./target/debug/vader llvm examples/db_sqlite.vd 2>&1 | tail -15
echo "=== 2nd time (.o cache + persistence to file) ==="
./target/debug/vader llvm examples/db_sqlite.vd 2>&1 | grep -vE 'emitted|compiled|linkando|compilando' | tail -6