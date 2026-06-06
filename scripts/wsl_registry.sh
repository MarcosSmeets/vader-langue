#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep -E 'test result' | head -1
BIN="$ROOT/target/debug/vader"

rm -rf /tmp/reg /tmp/greeter /tmp/app2 "$HOME/.vader/pkg" "$HOME/.vader/registry"
mkdir -p /tmp/reg

echo "=== lib 'greeter' (repo git com tag + origin) ==="
mkdir -p /tmp/greeter
printf 'public fn Greet(): string {\n    return "ola do pacote do registro!"\n}\n' > /tmp/greeter/greeter.vd
printf 'name = "greeter"\n' > /tmp/greeter/vader.toml
git -C /tmp/greeter init -q
git -C /tmp/greeter add -A
git -C /tmp/greeter -c user.email=a@b.c -c user.name=t commit -qm v1
git -C /tmp/greeter tag v1.0.0
git -C /tmp/greeter remote add origin /tmp/greeter

echo "=== vader publish (greeter -> registro /tmp/reg) ==="
cd /tmp/greeter
"$BIN" publish --registry /tmp/reg
echo "--- index.json ---"; cat /tmp/reg/index.json; echo

echo "=== projeto que faz 'vader add greeter' POR NOME ==="
mkdir -p /tmp/app2
printf 'import "greeter"\n\npublic fn main() {\n    print(greeter.Greet())\n}\n' > /tmp/app2/main.vd
printf 'name = "app2"\n' > /tmp/app2/vader.toml
cd /tmp/app2
"$BIN" add greeter --registry /tmp/reg
echo "--- app2/vader.toml ---"; cat vader.toml
echo "=== vader run . ==="
"$BIN" run .