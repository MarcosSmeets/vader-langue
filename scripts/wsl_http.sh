#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep 'test result' | head -1
BIN=./target/debug/vader

echo "=== compiles http/json runtimes ==="
clang -c -O2 -Wall runtime/vader_http.c -o /tmp/h.o 2>/tmp/h.log && echo "http OK" || { echo http FAILED; grep error: /tmp/h.log|head; }
clang -c -O2 -Wall runtime/vader_json.c -o /tmp/j.o 2>/tmp/j.log && echo "json OK" || { echo json FAILED; grep error: /tmp/j.log|head; }

echo "=== json_demo (parse + build + encode) ==="
"$BIN" llvm examples/json_demo.vd 2>&1 | grep -vE 'emitted|compiled|linkando|compilando' | tail -3

echo "=== starts the HTTP server in the background ==="
pkill -f 'vader_llvm/out' 2>/dev/null
"$BIN" llvm examples/http_server.vd >/tmp/srv.log 2>&1 &
sleep 9
echo "--- curl http://127.0.0.1:8081/hello ---"
curl -s http://127.0.0.1:8081/hello; echo

echo "=== Vader's own HTTP client (http.get) ==="
cat > /tmp/client.vd <<'EOF'
import "std/http"
public fn main() {
    string r = http.get("http://127.0.0.1:8081/from-vader")
    print(r)
}
EOF
"$BIN" llvm /tmp/client.vd 2>&1 | grep -vE 'emitted|compiled|linkando|compilando' | tail -2

pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== done ==="