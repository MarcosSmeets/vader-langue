#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== compile vader_router.c ==="
clang -c -O2 -Wall runtime/vader_router.c -o /tmp/vr.o 2>/tmp/vr.log && echo "router OK" || { echo FAILED; grep error: /tmp/vr.log | head; }
echo "=== run the router example + curl ==="
pkill -f 'vader_llvm/out' 2>/dev/null
"$BIN" llvm examples/api_router.vd >/tmp/router.log 2>&1 &
sleep 9
echo "--- GET /users ---"; curl -s http://127.0.0.1:8095/users; echo
echo "--- POST /users ---"; curl -s -X POST http://127.0.0.1:8095/users; echo
echo "--- GET /missing (should 404) ---"; curl -s http://127.0.0.1:8095/missing; echo
echo "--- build log errors? ---"; grep -iE 'error|unknown|type error' /tmp/router.log | head
pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== done ==="