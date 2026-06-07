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
rm -rf /tmp/vader_llvm
echo "=== first build (cold cache) ==="
s=$(date +%s%N); "$BIN" llvm --out /tmp/srv1 . >/tmp/b1.log 2>&1; r=$?; e=$(date +%s%N)
awk "BEGIN{printf \"  %.1f s (exit $r)\n\", ($e-$s)/1e9}"
echo "=== second build (warm cache) ==="
s=$(date +%s%N); "$BIN" llvm --out /tmp/srv2 . >/tmp/b2.log 2>&1; r=$?; e=$(date +%s%N)
awk "BEGIN{printf \"  %.1f s (exit $r)\n\", ($e-$s)/1e9}"
echo "=== run + curl ==="
rm -f /tmp/demo.db
DATABASE_URL=/tmp/demo.db /tmp/srv2 >/tmp/run.log 2>&1 &
sleep 2
curl -s -m 3 http://127.0.0.1:8080/health; echo " [health]"
curl -s -m 3 -X POST -d '{"name":"Caching"}' http://127.0.0.1:8080/users; echo " [post]"
curl -s -m 3 http://127.0.0.1:8080/users; echo " [list]"
pkill -f /tmp/srv2 2>/dev/null
echo "--- first build tail ---"; tail -4 /tmp/b1.log