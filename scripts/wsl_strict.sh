#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
BIN="$ROOT/target/debug/vader"
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -40
echo "=== cargo test ==="
cargo test --quiet 2>&1 | grep -E 'test result|error\[' | head -5
echo "=== check on the pure examples (no imports) — should PASS ==="
for f in hello basics shapes math calc generics generics_demo interfaces slices maps concurrency; do
  out=$("$BIN" check examples/$f.vd 2>&1)
  if echo "$out" | grep -qiE 'error|unknown'; then echo "  REGRESSION $f -> $(echo "$out" | head -1)"; else echo "  ok $f"; fi
done
echo "=== stdlib resolves (via llvm: DB/Json types must not become 'unknown') ==="
"$BIN" llvm examples/json_demo.vd 2>&1 | grep -iE 'unknown|type error' | head -2; echo "json_demo checked"
rm -f /tmp/vader_demo.db; "$BIN" llvm examples/db_sqlite.vd 2>&1 | grep -iE 'unknown|type error' | head -2; echo "db_sqlite checked"
echo "=== scaffold: vader new api + check (multi-file flatten) ==="
cd /tmp && rm -rf loja && "$BIN" new api loja >/dev/null 2>&1 && "$BIN" check loja 2>&1 | head -5 && echo "(scaffold ok if no error above)"
echo "=== NEGATIVE: unknown type should ERROR ==="
printf 'fn main() {\n    Foo x = 0\n}\n' > /tmp/bad.vd
"$BIN" check /tmp/bad.vd 2>&1 | head -2