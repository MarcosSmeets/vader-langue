#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== regression: slice/stdlib/concurrency/worker examples ==="
for f in slices stdlib_demo concurrency worker_mem; do
  echo "--- $f ---"
  "$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running' | head -2
done
echo "=== RANGE loop: 1,000,000 iters allocating JSON -> flat memory + correct ==="
cat > /tmp/rmem.vd <<'EOF'
import "std/json"
public fn main() {
    int total = 0
    for i in 0..1000000 {
        Json m = json.object()
        json.set_int(m, "n", i)
        total = total + json.as_int(json.field(m, "n"))
    }
    print("total:", total)
}
EOF
"$BIN" llvm --out /tmp/rmem /tmp/rmem.vd >/dev/null 2>&1
/usr/bin/time -v /tmp/rmem 2>&1 | grep -E 'total:|Maximum resident'
echo "=== SLICE for-in correctness (heap element var) + soundness (escape) ==="
cat > /tmp/smem.vd <<'EOF'
import "std/strings"
public fn main() {
    []string xs = strings.split("alpha,beta,gamma", ",")
    for x in xs {
        print(strings.upper(x))
    }
    string acc = ""
    for y in xs {
        acc = acc + y
    }
    print("acc:", acc)
}
EOF
"$BIN" llvm /tmp/smem.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running' | tail -5