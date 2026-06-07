#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1

echo "=== regression: worker_mem + basics still correct ==="
"$BIN" llvm examples/worker_mem.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running' | head -2
"$BIN" llvm examples/basics.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running'

echo "=== flat memory: 1,000,000 iterations allocating JSON per pass ==="
cat > /tmp/loopmem.vd <<'EOF'
import "std/json"
public fn main() {
    int i = 0
    int total = 0
    for i < 1000000 {
        Json msg = json.object()
        json.set_int(msg, "n", i)
        int got = json.as_int(json.field(msg, "n"))
        total = total + got
        i = i + 1
    }
    print("total:", total)
}
EOF
"$BIN" llvm --out /tmp/loopmem /tmp/loopmem.vd >/dev/null 2>&1
echo "--- run under /usr/bin/time (expect total=499999500000 + tiny peak RSS) ---"
/usr/bin/time -v /tmp/loopmem 2>&1 | grep -E 'total:|Maximum resident'

echo "=== soundness: a loop that ESCAPES (accumulates into an outer string) ==="
cat > /tmp/escape.vd <<'EOF'
public fn main() {
    string acc = ""
    int i = 0
    for i < 5 {
        acc = acc + "x"
        i = i + 1
    }
    print(acc)
}
EOF
"$BIN" llvm /tmp/escape.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running' | tail -2