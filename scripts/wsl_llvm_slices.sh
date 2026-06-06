#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -30
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|FAILED' | head -3
BIN="$PWD/target/debug/vader"
echo "=== vader llvm examples/slices.vd ==="
"$BIN" llvm examples/slices.vd 2>&1 | tail -5
