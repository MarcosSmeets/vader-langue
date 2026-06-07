#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
cargo build 2>&1 | grep -E '^error' | head

echo "=== (1) multiple type errors in one file: are ALL reported? ==="
cat > /tmp/e1.vd <<'EOF'
public fn main() {
    int x = unknownA
    string y = unknownB
    Foo z = 0
}
EOF
"$BIN" check /tmp/e1.vd 2>&1 | head -6

echo "=== (2) unknown variable message ==="
cat > /tmp/e2.vd <<'EOF'
public fn main() {
    print(notdefined)
}
EOF
"$BIN" check /tmp/e2.vd 2>&1 | head -3

echo "=== (3) multi-file project: does the error name the FILE? ==="
rm -rf /tmp/eproj && mkdir -p /tmp/eproj
printf 'name = "eproj"\n' > /tmp/eproj/vader.toml
printf 'public fn main() {\n    helper()\n}\n' > /tmp/eproj/main.vd
printf 'public fn helper() {\n    Bogus b = 0\n}\n' > /tmp/eproj/helper.vd
"$BIN" check /tmp/eproj 2>&1 | head -4

echo "=== (4) parse error ==="
cat > /tmp/e3.vd <<'EOF'
public fn main() {
    int x =
}
EOF
"$BIN" check /tmp/e3.vd 2>&1 | head -3