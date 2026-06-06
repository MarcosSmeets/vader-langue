#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -40
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|error\[|FAILED|panicked' | head -10
echo "=== native examples (incl. translated C runtime) ==="
for f in basics shapes slices interfaces generics_demo maps concurrency json_demo worker_mem; do
  out=$("$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiled|linkando|linking|compilando|compiling' | grep -v '^--- running' | tr '\n' '|')
  echo "  $f -> $out"
done
echo "=== db_sqlite (translated db runtime) ==="
rm -f /tmp/vader_demo.db
"$BIN" llvm examples/db_sqlite.vd 2>&1 | grep -vE 'emitted|compiled|linking|compiling' | tail -3
echo "=== http server smoke ==="
pkill -f 'vader_llvm/out' 2>/dev/null
"$BIN" llvm examples/http_server.vd >/tmp/srv.log 2>&1 &
sleep 8
curl -s http://127.0.0.1:8081/x; echo
pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== any PT-BR strings left in the code? (sample) ==="
grep -rniE 'função|não |conexão|erro |arquivo|gerado|liberação|servidor|usuário' src/ runtime/ 2>/dev/null | grep -vE '//|/\*| \* ' | head -10
echo "=== done ==="