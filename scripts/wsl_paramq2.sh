#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
BIN="$ROOT/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head
cargo test --quiet 2>&1 | grep 'test result' | head -1
cd /tmp && rm -rf demo
"$BIN" new api demo </dev/null >/dev/null 2>&1
cd /tmp/demo
export DATABASE_URL="/tmp/demo.db"; rm -f /tmp/demo.db
pkill -f 'vader_llvm/out' 2>/dev/null; sleep 1
"$BIN" llvm . >/tmp/demo.log 2>&1 &
echo "compiling (waiting 45s)..."; sleep 45
echo "=== build log ==="; cat /tmp/demo.log
echo "=== server alive? ==="; pgrep -f 'vader_llvm/out' >/dev/null && echo ALIVE || echo DEAD
echo "=== curls ==="
curl -s -m 3 http://127.0.0.1:8080/health; echo " [health]"
curl -s -m 3 -X POST -d '{"name":"O'"'"'Brien"}' http://127.0.0.1:8080/users; echo " [post O'\''Brien]"
curl -s -m 3 -X POST -d '{"name":"Ada"}' http://127.0.0.1:8080/users; echo " [post Ada]"
curl -s -m 3 http://127.0.0.1:8080/users; echo " [list]"
pkill -f 'vader_llvm/out' 2>/dev/null