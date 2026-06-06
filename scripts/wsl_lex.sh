#!/usr/bin/env bash
# Demo: roda a CLI `vader lex` num arquivo de exemplo, dentro do WSL.
set -u
export PATH="$HOME/.cargo/bin:$PATH"
PROJECT="/mnt/c/Users/marco/Documents/workspace/side_projects/vader"
cd "$PROJECT" || exit 1

FILE="${1:-examples/basics.vd}"
echo "=== vader lex $FILE ==="
cargo run --quiet -- lex "$FILE" 2>&1
