#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

DEMO=/tmp/vader_lint_demo
rm -rf "$DEMO"; mkdir -p "$DEMO"; cd "$DEMO" || exit 1

# projeto clean
"$BIN" new api loja --arch clean >/dev/null
cd loja || exit 1

echo "### 1) lint num arquivo correto (domain/user.vd)"
"$BIN" lint domain/user.vd
echo "exit: $?"

echo
echo "### 2) dev comete o erro: domain importando infra"
cat > domain/leak.vd <<'EOF'
import "loja/infra/db"

public struct Leak {
    id int
}
EOF
"$BIN" lint domain/leak.vd
echo "exit: $?  (!= 0 => barra o build/push)"
