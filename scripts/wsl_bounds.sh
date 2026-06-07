#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== existing slice examples still run (in-bounds) ==="
for f in slices stdlib_demo; do
  echo "--- $f ---"
  "$BIN" llvm examples/$f.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running' | head -4
done
echo "=== out-of-bounds READ must panic with the line + exit 1 ==="
cat > /tmp/oob.vd <<'EOF'
public fn main() {
    []int xs = [10, 20, 30]
    print("in bounds:", xs[1])
    int i = 5
    print("oob:", xs[i])
}
EOF
"$BIN" llvm /tmp/oob.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running'
echo "  (exit code: $?)"
echo "=== out-of-bounds WRITE must panic ==="
cat > /tmp/oobw.vd <<'EOF'
public fn main() {
    []int xs = [1, 2, 3]
    xs[10] = 99
}
EOF
"$BIN" llvm /tmp/oobw.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running'