#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1

cargo build --quiet
BIN="$PWD/target/debug/vader"

DEMO=/tmp/vader_gen_demo
rm -rf "$DEMO"; mkdir -p "$DEMO"; cd "$DEMO" || exit 1

echo "### vader gen fn somar"
"$BIN" gen fn somar
echo "### vader gen usecase CreateOrder"
"$BIN" gen usecase CreateOrder
echo
echo "=== generated tree ==="
find . -type f | sort
echo "=== somar_test.vd (born on its own) ==="
cat somar_test.vd
echo "=== usecase/create_order.vd ==="
cat usecase/create_order.vd
