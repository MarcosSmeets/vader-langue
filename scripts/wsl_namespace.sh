#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
BIN="$ROOT/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep 'test result' | head -1

echo "=== set up greeter dep (defines greet() AND User — same names as the app) ==="
rm -rf /tmp/greeter /tmp/myapp "$HOME/.vader/pkg"
mkdir -p /tmp/greeter
cat > /tmp/greeter/greeter.vd <<'EOF'
public fn greet(): string {
    return "hi from greeter"
}
public struct User {
    id int
}
EOF
printf 'name = "greeter"\n' > /tmp/greeter/vader.toml
git -C /tmp/greeter init -q
git -C /tmp/greeter add -A
git -C /tmp/greeter -c user.email=a@b.c -c user.name=t commit -qm v1

echo "=== app with its OWN greet() AND User, importing greeter ==="
mkdir -p /tmp/myapp
cat > /tmp/myapp/main.vd <<'EOF'
import "greeter"

public fn greet(): string {
    return "my own greet"
}
public struct User {
    name string
}
public fn main() {
    print(greet())
    print(greeter.greet())
    User u = User{ name: "Marco" }
    print(u.name)
    greeter.User g = greeter.User{ id: 7 }
    print(g.id)
}
EOF
printf 'name = "myapp"\n' > /tmp/myapp/vader.toml
cd /tmp/myapp
"$BIN" add /tmp/greeter >/dev/null 2>&1
echo "--- vader run . (expect: my own greet / hi from greeter / Marco / 7) ---"
"$BIN" run . 2>&1 | tail -8