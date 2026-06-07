#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== all examples at -O2 (UB regression check) ==="
for f in basics shapes slices interfaces generics_demo maps concurrency json_demo worker_mem; do
  out=$("$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiled|linking|compiling|built' | grep -v '^--- running' | tr '\n' '|')
  echo "  $f -> $out"
done
rm -f /tmp/vader_demo.db
"$BIN" llvm examples/db_sqlite.vd 2>&1 | grep -vE 'emitted|compiled|linking|compiling' | tail -2
echo "=== compilers available for benchmarks? ==="
for c in go rustc clang clang++ gcc g++; do
  command -v $c >/dev/null && echo "  $c: $($c --version 2>&1 | head -1)" || echo "  $c: MISSING"
done