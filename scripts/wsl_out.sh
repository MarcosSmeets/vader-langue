#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
BIN="$ROOT/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1

cd /tmp && rm -rf demo
"$BIN" new api demo --arch tdd --db sqlite >/dev/null 2>&1
cd /tmp/demo
echo "=== generated Dockerfile ==="
cat Dockerfile
echo "=== vader llvm --out /tmp/server . (build only, must NOT block) ==="
rm -f /tmp/server
timeout 40 "$BIN" llvm --out /tmp/server . 2>&1 | tail -3
echo "binary exists? $([ -f /tmp/server ] && echo YES || echo NO)"
echo "=== run the built binary + curl ==="
pkill -f '/tmp/server' 2>/dev/null
rm -f /tmp/d.db
DATABASE_URL=/tmp/d.db /tmp/server >/tmp/srv.log 2>&1 &
sleep 2
curl -s -m 3 http://127.0.0.1:8080/health; echo " [health]"
curl -s -m 3 -X POST -d '{"name":"Grace"}' http://127.0.0.1:8080/users; echo " [post]"
curl -s -m 3 http://127.0.0.1:8080/users; echo " [list]"
pkill -f '/tmp/server' 2>/dev/null
echo "=== done ==="