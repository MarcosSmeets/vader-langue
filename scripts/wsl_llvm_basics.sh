#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E "test result|error\[|FAILED|panicked" | head -10
BIN="$PWD/target/debug/vader"
echo "=== vader llvm examples/basics.vd ==="
"$BIN" llvm examples/basics.vd
echo "exit=$?"
echo "=== (if clang failed) generated IR: ==="
test -f /tmp/vader_llvm/out.ll && wc -l /tmp/vader_llvm/out.ll
