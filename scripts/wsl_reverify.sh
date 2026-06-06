#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== examples still run ==="
for f in basics shapes json_demo worker_mem; do
  out=$("$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiled|linking|compiling' | grep -v '^--- running' | tr '\n' '|')
  echo "  $f -> $out"
done
echo "=== std/env smoke (env.read) ==="
cat > /tmp/envtest.vd <<'EOF'
import "std/env"
public fn main() {
    string v = env.read("VADER_TEST")
    print("VADER_TEST =", v)
}
EOF
VADER_TEST="it-works" "$BIN" llvm /tmp/envtest.vd 2>&1 | grep -vE 'emitted|compiled|linking|compiling' | tail -2