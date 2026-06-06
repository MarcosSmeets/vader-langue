#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
BIN=/mnt/c/Users/marco/Documents/workspace/side_projects/vader/target/debug/vader
cd /tmp/vader_clean_demo || exit 1

echo "=== vader build loja ==="
"$BIN" build loja
echo "exit=$?"
echo "=== arquivos ==="
ls -la loja/loja 2>/dev/null && echo "(binário existe)" || echo "(sem binário loja/loja)"
ls -la ./loja 2>/dev/null | head -1
echo "=== procurando binário ELF ==="
find . -maxdepth 2 -type f -name loja -exec file {} \;
