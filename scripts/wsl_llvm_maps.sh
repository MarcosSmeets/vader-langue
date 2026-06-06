#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -40
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|FAILED' | head -3
BIN="$PWD/target/debug/vader"
echo "=== vader llvm examples/maps.vd ==="
"$BIN" llvm examples/maps.vd 2>&1 | tail -6
echo "=== reconfirm all ==="
for f in basics shapes slices interfaces generics_demo concurrency; do
  printf "%s: " "$f"; "$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiled|running|linkando' | tr '\n' ' '; echo
done
