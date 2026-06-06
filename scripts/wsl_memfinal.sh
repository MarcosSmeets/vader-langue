#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== regressão (basics/shapes/json/concurrency) ==="
for f in basics shapes json_demo concurrency; do
  out=$("$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiled|linkando|compilando' | grep -v '^--- running' | tr '\n' '|')
  echo "  $f -> $out"
done
echo "=== servidor: RSS por 1000 requests (deve ESTABILIZAR agora) ==="
pkill -f 'vader_llvm/out' 2>/dev/null
"$BIN" llvm examples/http_server.vd >/tmp/srv.log 2>&1 &
sleep 9
PID=$(pgrep -f 'vader_llvm/out' | head -1)
rss() { awk '/VmRSS/{print $2}' /proc/$PID/status 2>/dev/null; }
echo "inicial: $(rss) kB"
for round in 1 2 3 4 5 6 7 8; do
  for i in $(seq 1 1000); do curl -s http://127.0.0.1:8081/x >/dev/null; done
  echo "após $((round * 1000)): $(rss) kB"
done
pkill -f 'vader_llvm/out' 2>/dev/null