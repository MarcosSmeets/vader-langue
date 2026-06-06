#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1

rm -rf /tmp/apitest
mkdir -p /tmp/apitest
printf 'name = "apitest"\n' > /tmp/apitest/vader.toml
cat > /tmp/apitest/main.vd <<'EOF'
import "std/http"
import "std/env"

public fn main() {
    string who = env.read("WHO")
    Server s = http.listen(8090)
    for http.accept(s) {
        handle(s, who)
    }
}
EOF
cat > /tmp/apitest/greet.vd <<'EOF'
import "std/http"

public fn handle(s Server, who string) {
    http.respond(s, 200, "text/plain", who)
}
EOF

echo "=== vader llvm <dir> — multi-file native build + run ==="
pkill -f 'vader_llvm/out' 2>/dev/null
WHO="hello-from-dir" "$BIN" llvm /tmp/apitest >/tmp/apit.log 2>&1 &
sleep 9
echo "--- curl http://127.0.0.1:8090/ ---"
curl -s http://127.0.0.1:8090/; echo
echo "--- build log (errors?) ---"
grep -iE 'error|type error|unknown' /tmp/apit.log | head -5
pkill -f 'vader_llvm/out' 2>/dev/null
echo "=== done ==="