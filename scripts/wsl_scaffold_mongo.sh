#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
cd /tmp && rm -rf demo
"$BIN" new api demo --db mongo </dev/null >/dev/null 2>&1
echo "--- handlers/users.vd (mongo?) ---"
grep -E 'mongo\.(connect|insert|find)' demo/handlers/users.vd
echo "--- .env.example ---"; cat demo/.env.example
cd /tmp/demo
export DATABASE_URL="mongodb://admin:secret@127.0.0.1:27017/demo"
pkill -f 'vader_llvm/out' 2>/dev/null; sleep 1
"$BIN" llvm . >/tmp/demo.log 2>&1 &
sleep 20
echo "--- build log tail ---"; tail -2 /tmp/demo.log
echo "--- health ---";        curl -s -m 3 http://127.0.0.1:8080/health; echo
echo "--- POST /users ---";   curl -s -m 3 -X POST -d '{"name":"Mongoose","age":7}' http://127.0.0.1:8080/users; echo
echo "--- POST /users 2 ---"; curl -s -m 3 -X POST -d '{"name":"Ada","age":36}' http://127.0.0.1:8080/users; echo
echo "--- GET /users ---";    curl -s -m 3 http://127.0.0.1:8080/users; echo
pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== done ==="