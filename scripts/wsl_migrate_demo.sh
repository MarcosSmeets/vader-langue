#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build --quiet
BIN="$PWD/target/debug/vader"

WORK=/tmp/vader_migrate_demo
rm -rf "$WORK"; mkdir -p "$WORK"; cd "$WORK" || exit 1
"$BIN" new api loja --arch clean >/dev/null
cd loja || exit 1

echo "### vader migrate gen create_users"
"$BIN" migrate gen create_users
echo "### conteúdo do .up.sql"
cat migrations/*.up.sql
echo "### vader migrate status"
"$BIN" migrate status
echo "### vader migrate up"
"$BIN" migrate up
echo "### status de novo"
"$BIN" migrate status
