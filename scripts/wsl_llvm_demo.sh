#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

echo "########## vader llvm examples/llvm_demo.vd ##########"
"$BIN" llvm examples/llvm_demo.vd
echo "=== tipo do binário ==="
file /tmp/vader_llvm/out 2>/dev/null
echo "=== trecho do LLVM IR gerado ==="
head -n 20 /tmp/vader_llvm/out.ll
