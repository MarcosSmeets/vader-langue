#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

echo "########## 1) vader test sem gate (arquivo solto) ##########"
"$BIN" test examples/calc.vd
echo "exit code: $?"

echo
echo "########## 2) com vader.toml exigindo 80% (gate liga) ##########"
DEMO=/tmp/vader_test_gate
rm -rf "$DEMO"; mkdir -p "$DEMO"; cp examples/calc.vd "$DEMO/calc.vd"
cat > "$DEMO/vader.toml" <<'EOF'
[project]
name = "calc"

[test]
coverage_gate = true
min_coverage  = 80
EOF
cd "$DEMO" || exit 1
"$BIN" test calc.vd
echo "exit code: $?  (!= 0 => git push seria bloqueado)"
