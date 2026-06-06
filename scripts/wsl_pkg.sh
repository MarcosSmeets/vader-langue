#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
echo "=== cargo build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep -E 'test result|error\[' | head -5
BIN="$ROOT/target/debug/vader"

echo "=== monta a lib 'greeter' como repo git local ==="
rm -rf /tmp/greeter /tmp/myapp "$HOME/.vader/pkg"
mkdir -p /tmp/greeter
printf 'public fn Hello(): string {\n    return "ola do pacote greeter!"\n}\n' > /tmp/greeter/greeter.vd
git -C /tmp/greeter init -q
git -C /tmp/greeter add -A
git -C /tmp/greeter -c user.email=a@b.c -c user.name=t commit -qm "greeter v1"

echo "=== monta o projeto 'myapp' que importa greeter ==="
mkdir -p /tmp/myapp
printf 'import "greeter"\n\npublic fn main() {\n    print(greeter.Hello())\n}\n' > /tmp/myapp/main.vd
printf 'name = "myapp"\n' > /tmp/myapp/vader.toml

cd /tmp/myapp
echo "=== vader add /tmp/greeter ==="
"$BIN" add /tmp/greeter
echo "--- vader.toml ---"; cat vader.toml
echo "--- vader.lock ---"; cat vader.lock
echo "=== vader check . (typecheck com a dep, sem Go) ==="
"$BIN" check .
echo "=== vader run . (build via Go + roda) ==="
"$BIN" run .