#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
BIN="$ROOT/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep 'test result' | head -1

echo "=== vader new api demo --arch tdd --db sqlite ==="
cd /tmp && rm -rf demo
"$BIN" new api demo --arch tdd --db sqlite
echo "--- generated files ---"
find demo -type f | sort

echo "=== build + run the generated API (vader llvm .) ==="
cd /tmp/demo
export DATABASE_URL="/tmp/demo.db"
rm -f /tmp/demo.db
pkill -f 'vader_llvm/out' 2>/dev/null; sleep 1
"$BIN" llvm . >/tmp/demo.log 2>&1 &
sleep 10
echo "--- GET /health ---";  curl -s -m 3 http://127.0.0.1:8080/health; echo
echo "--- POST /users {name:Ada} ---"; curl -s -m 3 -X POST -d '{"name":"Ada"}' http://127.0.0.1:8080/users; echo
echo "--- POST /users {name:Linus} ---"; curl -s -m 3 -X POST -d '{"name":"Linus"}' http://127.0.0.1:8080/users; echo
echo "--- GET /users ---";  curl -s -m 3 http://127.0.0.1:8080/users; echo
echo "--- build/run log (errors?) ---"; grep -iE 'error|unknown|type error|listening' /tmp/demo.log | head
pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== done ==="