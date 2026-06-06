#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

WORK=/tmp/vader_module_demo
rm -rf "$WORK"; mkdir -p "$WORK"; cd "$WORK" || exit 1

echo "########## 1) scaffold minimal -> vader run no DIRETÓRIO ##########"
"$BIN" new cli demo --arch minimal >/dev/null
"$BIN" run demo
echo

echo "########## 2) projeto domain + cmd com tipos qualificados ##########"
mkdir -p proj/cmd proj/domain
cat > proj/domain/user.vd <<'EOF'
public struct User {
    id   int
    name string
}

public fn describe(u User): string {
    return "user " + u.name
}
EOF
cat > proj/cmd/main.vd <<'EOF'
import "proj/domain"

public fn main() {
    domain.User u = domain.User{ id: 1, name: "Ada" }
    print(domain.describe(u))
}
EOF
echo "--- estrutura ---"
find proj -type f | sort
echo "--- vader run proj ---"
"$BIN" run proj
