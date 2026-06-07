#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
BIN="$ROOT/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1

echo "=== vader new api demo (no --arch -> default tdd; non-tty -> sqlite) ==="
cd /tmp && rm -rf demo
"$BIN" new api demo </dev/null
echo "--- createUser (should use prepare/bind) ---"
grep -A2 'db.prepare' demo/handlers/users.vd

echo "=== build + run + test parameterized insert (name with a quote) ==="
cd /tmp/demo
export DATABASE_URL="/tmp/demo.db"; rm -f /tmp/demo.db
pkill -f 'vader_llvm/out' 2>/dev/null; sleep 1
"$BIN" llvm . >/tmp/demo.log 2>&1 &
sleep 25
echo "--- health ---"; curl -s -m 3 http://127.0.0.1:8080/health; echo
echo "--- POST name=O'Brien (tests SQL escaping) ---"; curl -s -m 3 -X POST -d '{"name":"O'"'"'Brien"}' http://127.0.0.1:8080/users; echo
echo "--- POST name=Ada ---"; curl -s -m 3 -X POST -d '{"name":"Ada"}' http://127.0.0.1:8080/users; echo
echo "--- GET /users (O'Brien must be intact, no injection) ---"; curl -s -m 3 http://127.0.0.1:8080/users; echo
pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== done ==="