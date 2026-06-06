#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -40
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|FAILED' | head -3
BIN="$PWD/target/debug/vader"
echo "=== vader llvm examples/generics_demo.vd ==="
"$BIN" llvm examples/generics_demo.vd 2>&1 | tail -5
echo "=== reconfirm the others (LLVM) ==="
for f in basics shapes slices interfaces; do
  echo "-- $f --"; "$BIN" llvm examples/$f.vd 2>&1 | tail -3
done
