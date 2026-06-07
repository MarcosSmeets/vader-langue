#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
cat > /tmp/assert.vd <<'EOF'
public fn main() {
    int x = 5
    assert x == 5
    print("passed first assert")
    assert x == 99
    print("should not reach here")
}
EOF
echo "=== assert (expect: 'passed first assert' then panic at line 5) ==="
"$BIN" llvm /tmp/assert.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached|running'