#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1

cargo build --quiet
BIN="$PWD/target/debug/vader"

DEMO=/tmp/vader_new_demo
rm -rf "$DEMO"; mkdir -p "$DEMO"; cd "$DEMO" || exit 1

"$BIN" new api loja --arch clean
echo "=== generated tree ==="
find loja -type f | sort
echo "=== loja/usecase/create_user.vd ==="
cat loja/usecase/create_user.vd
