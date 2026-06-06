#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

DEMO=/tmp/vader_spans_demo
rm -rf "$DEMO"; mkdir -p "$DEMO"; cd "$DEMO" || exit 1

cat > bug.vd <<'EOF'
fn add(a, b int): int {
    return a + b
}

fn main() {
    int total = add(1, "dois")
    if total {
        print(total)
    }
}
EOF

echo "=== bug.vd ==="
cat -n bug.vd
echo "=== vader check bug.vd ==="
"$BIN" check bug.vd
