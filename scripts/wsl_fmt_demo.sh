#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

DEMO=/tmp/vader_fmt_demo
rm -rf "$DEMO"; mkdir -p "$DEMO"; cd "$DEMO" || exit 1

cat > messy.vd <<'EOF'
public   struct User{id int
name string}
private fn   divide(a int,b int):(int,error){
if b==0{return 0,error("zero")}
return a/b,nil}
EOF

echo "=== ANTES (messy.vd) ==="
cat messy.vd
echo "=== DEPOIS (vader fmt) ==="
"$BIN" fmt messy.vd
