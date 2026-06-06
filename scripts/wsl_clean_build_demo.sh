#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

WORK=/tmp/vader_clean_demo
rm -rf "$WORK"; mkdir -p "$WORK"; cd "$WORK" || exit 1

"$BIN" new api loja --arch clean >/dev/null
echo "### vader build loja"
"$BIN" build loja
echo "exit=$?"
echo "### type of generated binary"
file loja/loja 2>/dev/null || echo "(no binary)"
echo "### running ./loja/loja"
./loja/loja
echo "### vader test loja"
"$BIN" test loja
