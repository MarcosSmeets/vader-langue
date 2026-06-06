#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -30
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|FAILED' | head -5
BIN="$PWD/target/debug/vader"
echo "=== vader llvm examples/basics.vd ==="
"$BIN" llvm examples/basics.vd 2>&1 | tail -4
echo "=== vader llvm examples/shapes.vd ==="
"$BIN" llvm examples/shapes.vd 2>&1 | tail -6
echo "=== vader llvm examples/llvm_demo.vd ==="
"$BIN" llvm examples/llvm_demo.vd 2>&1 | tail -5
