#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

# clean up previous template if it exists
rm -rf "$HOME/.vader/templates/meu-padrao"

WORK=/tmp/vader_tmpl_demo
rm -rf "$WORK"; mkdir -p "$WORK"; cd "$WORK" || exit 1

# 1) the dev sets up THEIR preferred structure, with __name__ and their libs
mkdir -p meu-padrao/src
cat > meu-padrao/vader.toml <<'EOF'
[project]
name = "__name__"

[dependencies]
http-extra = "1.2.0"
auth-jwt   = "0.3.1"
EOF
cat > meu-padrao/src/main.vd <<'EOF'
import "__name__/src"

public fn main() {
    print("bem-vindo ao __name__")
}
EOF

echo "### vader template save meu-padrao ./meu-padrao"
"$BIN" template save meu-padrao ./meu-padrao
echo
echo "### vader template list"
"$BIN" template list
echo
echo "### vader new --template meu-padrao cliente-x"
"$BIN" new --template meu-padrao cliente-x
echo
echo "=== cliente-x/vader.toml (placeholders replaced) ==="
cat cliente-x/vader.toml
echo "=== cliente-x/src/main.vd ==="
cat cliente-x/src/main.vd
